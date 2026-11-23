// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The window's side of the plugin host: the core functions plugins call (`WindowCore`), the core's own tools, and plugins withdrawn while running.
//!
//! Split out of main.rs (23 Nov 2026) as it was, without changes: it
//! uses main.rs's items and imports through `use super::*`.

use super::*;

/// What the window does for plugins: shows their footer messages, shows
/// their tabs again when they ask, and hands them the open score. Requests
/// may come from any thread (a plugin runs on the thread that called it),
/// so the first two are handed to the event loop.
pub(crate) struct WindowCore {
    pub(crate) open: OpenScore,
    /// The fonts plugins measure their surfaces' text with (interface
    /// 1.14): the ones their text is drawn in.
    pub(crate) fonts: Arc<oxiguitar_display::FontSet>,
}

/// The window's thread: a plugin asking from it to run a tool or select
/// would wait on itself (interface 1.14).
pub(crate) static UI_THREAD: std::sync::OnceLock<std::thread::ThreadId> =
    std::sync::OnceLock::new();

/// Why a plugin's request to run a tool or select can't wait here, if it
/// can't: on the window's thread or the worker's (inside a tool's own run),
/// the answer would wait for the thread that waits for it.
pub(crate) fn cannot_wait() -> Option<String> {
    let here = std::thread::current();
    (UI_THREAD.get() == Some(&here.id()) || here.name() == Some("oxiguitar-worker")).then(|| {
        "it can't be asked from inside a tool's run or a drawing's click: ask from a thread of the plugin's own".to_owned()
    })
}

impl oxiguitar_host::Core for WindowCore {
    fn notify(&self, level: Level, text: String) {
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(app) = APP.with(|a| a.borrow().clone()) {
                app.borrow_mut().footer.push(level, text);
                sync_footer(&app);
            }
        });
    }

    fn notify_with_link(
        &self,
        level: Level,
        text: String,
        link: oxiguitar_host::api::message::Link,
    ) {
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(app) = APP.with(|a| a.borrow().clone()) {
                app.borrow_mut()
                    .footer
                    .push_with_link(level, text, link.text, link.url);
                sync_footer(&app);
            }
        });
    }

    fn advise(&self, asked: oxiguitar_host::advice::Asked) {
        // Plugins ask from any thread, even before the window exists: the
        // request waits in a line, and the window takes it when it is up.
        advice::ask(asked);
        let _ = slint::invoke_from_event_loop(|| {
            if let Some(app) = APP.with(|a| a.borrow().clone()) {
                show_advice(&app);
            }
        });
    }

    fn redraw(&self, tab: &TabId) {
        let tab = tab.clone();
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(app) = APP.with(|a| a.borrow().clone()) {
                plugin_tab_changed(&app, tab);
            }
        });
    }

    fn engraving(&self) -> Option<oxiguitar_host::api::message::Engraving> {
        self.open.engraving()
    }

    fn selection(&self) -> Option<oxiguitar_host::api::event::Selection2> {
        self.open.selection()
    }

    fn score_file(&self) -> Option<PathBuf> {
        self.open.file()
    }

    fn open_dialog(&self, tab: &TabId, title: &str) -> Result<(), String> {
        // Asked from any thread: whether one is open is known here, and
        // the window shows it when its turn comes.
        if DIALOG_OPEN.swap(true, std::sync::atomic::Ordering::SeqCst) {
            return Err("another dialog is open".to_owned());
        }
        let (tab, title) = (tab.clone(), title.to_owned());
        let queued =
            slint::invoke_from_event_loop(move || match APP.with(|a| a.borrow().clone()) {
                Some(app) => open_plugin_dialog(&app, tab, &title),
                None => DIALOG_OPEN.store(false, std::sync::atomic::Ordering::SeqCst),
            });
        queued.map_err(|_| {
            DIALOG_OPEN.store(false, std::sync::atomic::Ordering::SeqCst);
            "the window is closing".to_owned()
        })
    }

    fn show_tab(&self, tab: &TabId) {
        let tab = tab.clone();
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(app) = APP.with(|a| a.borrow().clone()) {
                plugin_tab_wanted(&app, &tab);
            }
        });
    }

    fn score(&self) -> Option<oxiguitar_host::api::score::Score> {
        self.open.wire()
    }

    fn score2(&self) -> Option<(u64, oxiguitar_host::api::score2::Score2)> {
        self.open.wire2()
    }

    fn apply(
        &self,
        plugin: &str,
        revision: u64,
        name: String,
        score: oxiguitar_host::api::score2::Score2,
    ) -> Result<(), String> {
        // Answered once the change is made, so the plugin reads the score
        // as changed (interface 1.14's plugins drive the editor), except
        // where waiting would wait on itself: inside a tool's run on the
        // worker, or on the window's thread.
        if cannot_wait().is_some() {
            return self.open.apply(plugin, revision, name, score, None);
        }
        let (reply, answer) = std::sync::mpsc::channel();
        self.open
            .apply(plugin, revision, name, score, Some(reply))?;
        answer
            .recv()
            .map_err(|_| "OxiGuitar is closing".to_owned())?
    }

    fn settings_changed(&self, id: &str) {
        let id = id.to_owned();
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(app) = APP.with(|a| a.borrow().clone()) {
                playing::settings_changed(&app, &id);
            }
        });
    }

    fn withdrawn(&self, id: &str) {
        let id = id.to_owned();
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(app) = APP.with(|a| a.borrow().clone()) {
                plugin_withdrawn(&app, &id);
            }
        });
    }

    fn measure_text(
        &self,
        text: &str,
        size: f32,
        font: oxiguitar_host::api::display::Font,
    ) -> Option<oxiguitar_host::api::message::TextMeasure> {
        let font = match font {
            oxiguitar_host::api::display::Font::Sans => oxiguitar_display::FontSet::TEXT,
            oxiguitar_host::api::display::Font::Music => oxiguitar_display::FontSet::MUSIC,
        };
        let (edges, ascent, descent) = self.fonts.measure(font, size, text);
        Some(oxiguitar_host::api::message::TextMeasure {
            edges,
            ascent,
            descent,
        })
    }

    fn copy(&self, text: String) {
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(ui) = APP
                .with(|a| a.borrow().clone())
                .and_then(|app| app.borrow().ui.upgrade())
            {
                ui.invoke_copy_text(text.into());
            }
        });
    }

    fn core_tools(&self) -> Vec<oxiguitar_host::api::message::ToolEntry> {
        automation::entries()
    }

    fn run_tool(
        &self,
        caller: &str,
        owner: &str,
        tool: &str,
        options: Option<String>,
    ) -> Result<String, String> {
        if let Some(why) = cannot_wait() {
            return Err(why);
        }
        let _ = caller;
        let (reply, answer) = std::sync::mpsc::channel();
        if owner == oxiguitar_host::api::message::CORE_TOOLS {
            let run = automation::parse(tool, options.as_deref())?;
            // Everything already asked of the worker first (a plugin's
            // change just handed back, say), so the tool runs on the score
            // as it is: the worker's answers reach the window's thread
            // before this request does.
            let (fenced, done) = std::sync::mpsc::channel();
            self.open.send(worker::Request::Fence {
                before: self.open.revision(),
                name: String::new(),
                reply: fenced,
            })?;
            let _ = done.recv();
            slint::invoke_from_event_loop(move || match APP.with(|a| a.borrow().clone()) {
                Some(app) => run_core_tool(&app, run, reply),
                None => {
                    let _ = reply.send(Err("the window is closing".to_owned()));
                }
            })
            .map_err(|_| "the window is closing".to_owned())?;
        } else {
            self.open.send(worker::Request::RunPluginTool {
                plugin: owner.to_owned(),
                tool: tool.to_owned(),
                options,
                reply,
            })?;
        }
        answer
            .recv()
            .map_err(|_| "the tool didn't run: OxiGuitar is closing".to_owned())?
    }

    fn select(&self, selection: oxiguitar_host::api::event::Selection2) -> Result<(), String> {
        if let Some(why) = cannot_wait() {
            return Err(why);
        }
        let (reply, answer) = std::sync::mpsc::channel();
        slint::invoke_from_event_loop(move || {
            let done = match APP.with(|a| a.borrow().clone()) {
                Some(app) => {
                    let done = app.borrow_mut().editing.select_for_plugin(selection);
                    if done.is_ok() {
                        caret_moved(&app);
                    }
                    done
                }
                None => Err("the window is closing".to_owned()),
            };
            let _ = reply.send(done);
        })
        .map_err(|_| "the window is closing".to_owned())?;
        answer
            .recv()
            .map_err(|_| "OxiGuitar is closing".to_owned())?
    }
}

/// Runs one of the core's tools as a plugin asked (interface 1.14), as its
/// menu entry would: with options at once, as one change the user can
/// undo; without, its dialog opens for the user. `reply` hears the report
/// once the worker has made the change.
pub(crate) fn run_core_tool(
    app: &Shared,
    run: automation::CoreRun,
    reply: std::sync::mpsc::Sender<Result<String, String>>,
) {
    use automation::CoreRun;
    let fail = |why: &str| {
        let _ = reply.send(Err(why.to_owned()));
    };
    let name = run.name();
    {
        let s = app.borrow();
        if s.editing.score().is_none() {
            return fail("no score is open");
        }
        if s.open_score.playing() {
            return fail("the score is playing: it can be changed once it is paused or stopped");
        }
        if s.question.is_some() || DIALOG_OPEN.load(std::sync::atomic::Ordering::SeqCst) {
            return fail("the window is asking the user something: try again once it is answered");
        }
    }
    let before = app.borrow().open_score.revision();
    let dialog = |app: &Shared, open: fn(&Shared)| {
        open(app);
        let _ = reply.send(Ok(format!("{name}'s dialog is open: the user decides.")));
    };
    match run {
        CoreRun::Tool(i) => app.borrow().worker.send(Request::ApplyTool(i)),
        CoreRun::Fingering(scope) => find_fingering(app, scope),
        CoreRun::Transpose(None) => {
            return dialog(app, |a| open_transpose(a, oxiguitar_edit::Scope::All));
        }
        CoreRun::Fit(None) => return dialog(app, |a| open_fit(a, true)),
        CoreRun::Harmonize(None) => return dialog(app, |a| open_harmonize(a, true)),
        CoreRun::Transpose(Some(t)) => {
            let typed = app
                .borrow_mut()
                .editing
                .tool(oxiguitar_edit::Command::Transpose(t));
            if let Some(typed) = typed {
                send_typed(app, typed);
            }
        }
        CoreRun::Fit(Some(fit)) => {
            let typed = app
                .borrow_mut()
                .editing
                .tool(oxiguitar_edit::Command::FitBars(fit));
            if let Some(typed) = typed {
                send_typed(app, typed);
            }
        }
        CoreRun::Harmonize(Some(h)) => harmonize_run(app, h),
    }
    app.borrow().worker.send(Request::Fence {
        before,
        name: name.to_owned(),
        reply,
    });
}

/// The plugin `id` failed and was withdrawn: its tabs go, as if it had
/// never been installed, and the plugin manager shows why.
pub(crate) fn plugin_withdrawn(app: &Shared, id: &str) {
    let gone = {
        let mut s = app.borrow_mut();
        s.plugin_tabs.retain(|tab, _| tab.plugin != id);
        plugin_tabs::remove_from(&mut s.shell, id)
    };
    if gone {
        sync_shell(app);
        for side in Side::ALL {
            show_plugin_tab(app, side);
        }
    }
    sync_manager(app);
}
