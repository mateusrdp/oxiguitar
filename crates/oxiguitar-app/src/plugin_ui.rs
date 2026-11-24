// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Plugins' own UI in Slint (plugin interface 1.15; #346): each plugin's
//! `.slint` file, compiled with Slint's interpreter and shown with the
//! window's own Slint, widgets and style. The core draws nothing of its
//! own in a plugin's views; it only puts them where the manifest says.
//!
//! - **Windows** (`place: window`) are top-level windows the plugin opens
//!   and closes; the user can close them too.
//! - **Panels** (an area's place) are tabs of the window's areas, and
//!   **track sections** and **export options** sit in the Track tab and
//!   the export dialog. They are embedded with Slint's `ComponentContainer`
//!   and `ComponentDefinition::create_embedded`, which Slint 1.19 marks as
//!   experimental (Mateus, 24 Nov 2026: use it, and follow Slint if it
//!   changes): the workspace's `.cargo/config.toml` turns it on for the
//!   window's own `.slint` files, and the interpreter's `internal` feature
//!   gives `create_embedded`.
//!
//! Everything here runs on the window's thread, where Slint lives. The
//! window never calls a plugin: asking for a `.slint` file and telling a
//! plugin about a view (made, a callback, closed) go to a thread of their
//! own, in order. The values a plugin sets are kept by view, so a view
//! made again (a window reopened, a panel shown again, another track)
//! starts as the plugin last left it.

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::mpsc::Sender;

use oxiguitar_host::api::view::{Place, TrackView, Value as ViewValue};
use oxiguitar_host::{Level, Plugins, ViewId};
use slint::{ComponentFactory, ComponentHandle, Model, ModelRc, VecModel};
use slint_interpreter::{CompilationResult, ComponentDefinition, ComponentInstance, Value};

/// The style the window's own `.slint` files are compiled with (build.rs).
const STYLE: &str = "fluent";

/// Something to do with the plugins, off the window's thread.
type Job = Box<dyn FnOnce(&Plugins) + Send>;

/// What to do once a plugin's `.slint` file is compiled.
type Then = Box<dyn FnOnce()>;

#[derive(Default)]
struct State {
    /// The plugins, for their manifests (no plugin is called from here).
    plugins: Option<Plugins>,
    /// The thread that calls the plugins.
    jobs: Option<Sender<Job>>,
    /// Each plugin's compiled `.slint` file, or why it can't be used.
    compiled: HashMap<String, Result<Rc<CompilationResult>, String>>,
    /// What waits for a plugin's file, asked for and not yet answered.
    waiting: HashMap<String, Vec<Then>>,
    /// The properties each view was last given.
    values: HashMap<ViewId, BTreeMap<String, ViewValue>>,
    /// Every view made, for setting its properties.
    live: HashMap<ViewId, Vec<slint::Weak<ComponentInstance>>>,
    /// The open windows.
    windows: HashMap<ViewId, ComponentInstance>,
    /// Dark Interface.
    dark: bool,
    /// The factory each area's panel was last given, by area number, so a
    /// panel shown again isn't made again.
    panels: [Option<(ViewId, ComponentFactory)>; 4],
    /// The factory each track section was given, by view and opening of
    /// the Track tab's sections.
    sections: HashMap<(ViewId, u64), ComponentFactory>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

fn with<R>(f: impl FnOnce(&mut State) -> R) -> R {
    STATE.with(|s| f(&mut s.borrow_mut()))
}

/// Starts the thread that calls the plugins for their views.
pub(crate) fn init(plugins: Plugins, dark: bool) {
    let (tx, rx) = std::sync::mpsc::channel::<Job>();
    let worker = plugins.clone();
    let started = std::thread::Builder::new()
        .name("oxiguitar-plugin-ui".into())
        .spawn(move || {
            for job in rx {
                job(&worker);
            }
        });
    with(|s| {
        s.plugins = Some(plugins);
        s.jobs = started.ok().map(|_| tx);
        s.dark = dark;
    });
}

fn send(job: impl FnOnce(&Plugins) + Send + 'static) {
    with(|s| {
        if let Some(jobs) = &s.jobs {
            let _ = jobs.send(Box::new(job));
        }
    });
}

/// Says something in the footer, once the window's thread is free (this
/// may run while the window's state is borrowed).
fn say(level: Level, text: String) {
    let _ = slint::invoke_from_event_loop(move || {
        if let Some(app) = crate::APP.with(|a| a.borrow().clone()) {
            app.borrow_mut().footer.push(level, text);
            crate::sync_footer(&app);
        }
    });
}

/// The name a view's plugin goes by in messages.
fn plugin_name(plugin: &str) -> String {
    with(|s| {
        s.plugins
            .as_ref()
            .and_then(|p| {
                p.found()
                    .iter()
                    .filter_map(|f| f.manifest.as_ref())
                    .find(|m| m.id == plugin)
                    .map(|m| m.name.clone())
            })
            .unwrap_or_else(|| plugin.to_owned())
    })
}

/// The component the view `id` shows and where, from its manifest.
pub(crate) fn spec_of(id: &ViewId) -> Option<(String, Place)> {
    with(|s| {
        let plugins = s.plugins.as_ref()?;
        let view = plugins.view(id)?;
        Some((view.spec.component.clone(), view.spec.place))
    })
}

/// Compiles a plugin's `.slint` file with the window's style.
fn compile(source: String) -> Result<CompilationResult, String> {
    let mut compiler = slint_interpreter::Compiler::default();
    compiler.set_style(STYLE.into());
    // Every exported component, not only windows: panels, track sections
    // and export options are embedded, so they don't inherit Window.
    compiler
        .compiler_configuration(i_slint_core::InternalToken)
        .components_to_generate = i_slint_compiler::ComponentSelection::AllExported;
    let result = spin_on(compiler.build_from_source(source, PathBuf::from("plugin.slint")));
    let errors: Vec<String> = result
        .diagnostics()
        .filter(|d| d.level() == slint_interpreter::DiagnosticLevel::Error)
        .map(|d| {
            let (line, column) = d.line_column();
            format!("line {line}, column {column}: {}", d.message())
        })
        .collect();
    if errors.is_empty() {
        Ok(result)
    } else {
        Err(errors.join("; "))
    }
}

/// Runs a future that needs no executor: the interpreter's compiler,
/// whose only waits are for files, and a plugin's source is in memory.
fn spin_on<F: std::future::Future>(future: F) -> F::Output {
    let mut future = std::pin::pin!(future);
    let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
    loop {
        if let std::task::Poll::Ready(v) = future.as_mut().poll(&mut cx) {
            return v;
        }
    }
}

/// The component `component` of `plugin`'s compiled file, if compiled.
fn definition(plugin: &str, component: &str) -> Option<ComponentDefinition> {
    with(|s| match s.compiled.get(plugin) {
        Some(Ok(result)) => result.component(component),
        _ => None,
    })
}

/// Runs `then` once `plugin`'s `.slint` file is compiled, at once if it
/// is; never if it can't be (which is said in the footer once).
fn when_compiled(plugin: &str, then: impl FnOnce() + 'static) {
    enum Next {
        Now,
        Never,
        Wait,
        Ask,
    }
    let next = with(|s| match s.compiled.get(plugin) {
        Some(Ok(_)) => Next::Now,
        Some(Err(_)) => Next::Never,
        None if s.waiting.contains_key(plugin) => Next::Wait,
        None => Next::Ask,
    });
    match next {
        Next::Now => then(),
        Next::Never => {}
        Next::Wait | Next::Ask => {
            with(|s| {
                s.waiting
                    .entry(plugin.to_owned())
                    .or_default()
                    .push(Box::new(then));
            });
            if matches!(next, Next::Ask) {
                let plugin = plugin.to_owned();
                send(move |plugins| {
                    let source = plugins.ui_source(&plugin);
                    let _ = slint::invoke_from_event_loop(move || compiled(&plugin, source));
                });
            }
        }
    }
}

/// `plugin`'s `.slint` file came: compiled, then what waited runs.
fn compiled(plugin: &str, source: Result<String, oxiguitar_host::api::message::Error>) {
    let result = source.map_err(|e| e.message).and_then(compile).map(Rc::new);
    if let Err(why) = &result {
        say(
            Level::Error,
            format!(
                "{}: its windows and panels can't be shown ({why}).",
                plugin_name(plugin)
            ),
        );
    }
    let ok = result.is_ok();
    let waiting = with(|s| {
        s.compiled.insert(plugin.to_owned(), result);
        s.waiting.remove(plugin).unwrap_or_default()
    });
    if ok {
        for then in waiting {
            then();
        }
    }
}

/// Converts a plugin's value for Slint.
pub(crate) fn to_slint(value: &ViewValue) -> Value {
    match value {
        ViewValue::Bool(b) => Value::Bool(*b),
        ViewValue::Number(n) => Value::Number(*n),
        ViewValue::String(s) => Value::String(s.as_str().into()),
        ViewValue::List(items) => Value::Model(ModelRc::new(VecModel::from(
            items.iter().map(to_slint).collect::<Vec<_>>(),
        ))),
        ViewValue::Struct(fields) => Value::Struct(
            fields
                .iter()
                .map(|(name, v)| (name.clone(), to_slint(v)))
                .collect(),
        ),
        ViewValue::Image {
            columns,
            rows,
            rgba,
        } => {
            let mut buffer = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::new(*columns, *rows);
            buffer.make_mut_bytes().copy_from_slice(rgba);
            Value::Image(slint::Image::from_rgba8_premultiplied(buffer))
        }
    }
}

/// Converts Slint's value for a plugin: a struct's fields by name,
/// enumerations and colours as text,
/// what has no plugin form (paths, easing curves) as nothing.
pub(crate) fn from_slint(value: &Value) -> ViewValue {
    match value {
        Value::Bool(b) => ViewValue::Bool(*b),
        Value::Number(n) => ViewValue::Number(*n),
        Value::String(s) => ViewValue::String(s.to_string()),
        Value::Model(model) => ViewValue::List(model.iter().map(|v| from_slint(&v)).collect()),
        // By field name: Slint keeps a struct's fields in no order.
        Value::Struct(fields) => {
            let mut fields: Vec<(String, ViewValue)> = fields
                .iter()
                .map(|(name, v)| (name.to_owned(), from_slint(v)))
                .collect();
            fields.sort_by(|a, b| a.0.cmp(&b.0));
            ViewValue::Struct(fields)
        }
        Value::EnumerationValue(_, v) => ViewValue::String(v.clone()),
        Value::Brush(brush) => {
            let c = brush.color();
            ViewValue::String(format!(
                "#{:02x}{:02x}{:02x}{:02x}",
                c.red(),
                c.green(),
                c.blue(),
                c.alpha()
            ))
        }
        Value::Image(image) => match image.to_rgba8_premultiplied() {
            Some(pixels) => ViewValue::Image {
                columns: pixels.width(),
                rows: pixels.height(),
                rgba: pixels.as_bytes().to_vec(),
            },
            None => ViewValue::List(Vec::new()),
        },
        _ => ViewValue::List(Vec::new()),
    }
}

/// Gives a view just made its callbacks, the window's colours and the
/// values its plugin last set.
fn wire(id: &ViewId, def: &ComponentDefinition, instance: &ComponentInstance) {
    for callback in def.callbacks() {
        let (id, name) = (id.clone(), callback.clone());
        let _ = instance.set_callback(&callback, move |args| {
            let args: Vec<ViewValue> = args.iter().map(from_slint).collect();
            let (id, name) = (id.clone(), name.clone());
            send(move |plugins| plugins.view_callback(&id, &name, args));
            Value::Void
        });
    }
    let dark = with(|s| s.dark);
    let _ = instance.set_global_property(
        "Palette",
        "color-scheme",
        Value::EnumerationValue(
            "ColorScheme".into(),
            if dark { "dark" } else { "light" }.into(),
        ),
    );
    let values: Vec<(String, ViewValue)> = with(|s| {
        s.values
            .get(id)
            .map(|v| v.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default()
    });
    apply(id, instance, &values);
    with(|s| {
        let live = s.live.entry(id.clone()).or_default();
        live.retain(|w| w.upgrade().is_some());
        live.push(instance.as_weak());
    });
}

/// Sets properties of one view made; what can't be set is said once in
/// the footer.
fn apply(id: &ViewId, instance: &ComponentInstance, values: &[(String, ViewValue)]) {
    for (name, value) in values {
        if let Err(e) = instance.set_property(name, to_slint(value)) {
            say(
                Level::Warning,
                format!(
                    "{}: the property {name:?} of its view {:?} can't be set ({e}).",
                    plugin_name(&id.plugin),
                    id.view
                ),
            );
        }
    }
}

/// The plugin sets properties of its view: kept, and given to every view
/// made of it.
pub(crate) fn set(id: &ViewId, values: Vec<(String, ViewValue)>) {
    let live: Vec<ComponentInstance> = with(|s| {
        let kept = s.values.entry(id.clone()).or_default();
        for (name, value) in &values {
            kept.insert(name.clone(), value.clone());
        }
        s.live
            .get(id)
            .map(|l| l.iter().filter_map(slint::Weak::upgrade).collect())
            .unwrap_or_default()
    });
    for instance in live {
        apply(id, &instance, &values);
    }
}

/// Opens the view's window, or brings it forward if open.
pub(crate) fn open_window(id: &ViewId) {
    if let Some(open) = with(|s| s.windows.get(id).map(ComponentInstance::clone_strong)) {
        let _ = open.show();
        return;
    }
    let Some((component, _)) = spec_of(id) else {
        return;
    };
    let id = id.clone();
    when_compiled(&id.plugin.clone(), move || {
        let Some(def) = definition(&id.plugin, &component) else {
            say(
                Level::Error,
                format!(
                    "{}: its .slint file has no component {component:?} for its view {:?}.",
                    plugin_name(&id.plugin),
                    id.view
                ),
            );
            return;
        };
        if with(|s| s.windows.contains_key(&id)) {
            return;
        }
        let instance = match def.create() {
            Ok(i) => i,
            Err(e) => {
                say(
                    Level::Error,
                    format!(
                        "{}: its window {:?} can't open ({e}).",
                        plugin_name(&id.plugin),
                        id.view
                    ),
                );
                return;
            }
        };
        wire(&id, &def, &instance);
        let closing = id.clone();
        instance.window().on_close_requested(move || {
            close_window(&closing);
            slint::CloseRequestResponse::HideWindow
        });
        if instance.show().is_ok() {
            with(|s| s.windows.insert(id.clone(), instance));
            send(move |plugins| plugins.view_shown(&id, None));
        }
    });
}

/// Closes the view's window, if open, and tells its plugin.
pub(crate) fn close_window(id: &ViewId) {
    let Some(window) = with(|s| s.windows.remove(id)) else {
        return;
    };
    let _ = window.hide();
    let id = id.clone();
    send(move |plugins| plugins.view_closed(&id, false));
}

/// A factory for an embedded view (a panel, a track section, export
/// options): each view it makes gets its callbacks and values, and its
/// plugin is told, with `track` for a track section. Until the plugin's
/// `.slint` file is compiled it makes nothing; `refresh` then gets a
/// factory that does, to put in place of this one.
pub(crate) fn factory(
    id: &ViewId,
    track: Option<TrackView>,
    refresh: impl FnOnce(ComponentFactory) + 'static,
) -> ComponentFactory {
    make_factory(id, track, Box::new(refresh))
}

/// [`factory`] with its `refresh` boxed: it calls itself.
fn make_factory(
    id: &ViewId,
    track: Option<TrackView>,
    refresh: Box<dyn FnOnce(ComponentFactory)>,
) -> ComponentFactory {
    let Some((component, _)) = spec_of(id) else {
        return ComponentFactory::default();
    };
    if definition(&id.plugin, &component).is_none() {
        let (id, track) = (id.clone(), track.clone());
        when_compiled(&id.plugin.clone(), move || {
            if spec_of(&id)
                .and_then(|(c, _)| definition(&id.plugin, &c))
                .is_some()
            {
                refresh(make_factory(&id, track, Box::new(|_| {})));
            }
        });
        return ComponentFactory::default();
    }
    let id = id.clone();
    ComponentFactory::new(move |ctx| {
        let def = definition(&id.plugin, &component)?;
        let instance = def.create_embedded(ctx).ok()?;
        wire(&id, &def, &instance);
        let (id, track) = (id.clone(), track.clone());
        send(move |plugins| plugins.view_shown(&id, track));
        Some(instance)
    })
}

/// Tells the plugin its embedded view is gone (the dialog it was in
/// closed), then runs `then` on the window's thread: what waited for the
/// plugin to have heard it (an export reading its options).
pub(crate) fn closed_then(id: &ViewId, accepted: bool, then: impl FnOnce() + Send + 'static) {
    let id = id.clone();
    send(move |plugins| {
        plugins.view_closed(&id, accepted);
        let _ = slint::invoke_from_event_loop(then);
    });
}

/// Dark Interface changed: every view made follows.
pub(crate) fn dark(on: bool) {
    let live: Vec<ComponentInstance> = with(|s| {
        s.dark = on;
        s.live
            .values()
            .flatten()
            .filter_map(slint::Weak::upgrade)
            .collect()
    });
    for instance in live {
        let _ = instance.set_global_property(
            "Palette",
            "color-scheme",
            Value::EnumerationValue(
                "ColorScheme".into(),
                if on { "dark" } else { "light" }.into(),
            ),
        );
    }
}

/// The plugin `plugin` was withdrawn: its windows close and its file is
/// forgotten; its panels go with its tabs.
pub(crate) fn withdrawn(plugin: &str) {
    let windows: Vec<ComponentInstance> = with(|s| {
        s.compiled.remove(plugin);
        s.waiting.remove(plugin);
        s.live.retain(|id, _| id.plugin != plugin);
        s.sections.retain(|(id, _), _| id.plugin != plugin);
        for p in &mut s.panels {
            if p.as_ref().is_some_and(|(id, _)| id.plugin == plugin) {
                *p = None;
            }
        }
        s.values.retain(|id, _| id.plugin != plugin);
        let gone: Vec<ViewId> = s
            .windows
            .keys()
            .filter(|id| id.plugin == plugin)
            .cloned()
            .collect();
        gone.iter().filter_map(|id| s.windows.remove(id)).collect()
    });
    for w in windows {
        let _ = w.hide();
    }
}

/// The factory for area `area`'s panel `id` (the area's number, 0 top, 1
/// bottom, 2 left, 3 right), or `None` when the area already has it.
/// `set` puts a factory in the window, now or once the plugin's file is
/// compiled.
pub(crate) fn panel(area: usize, id: &ViewId, set: Rc<dyn Fn(ComponentFactory)>) {
    if with(|s| s.panels[area].as_ref().is_some_and(|(v, _)| v == id)) {
        return;
    }
    let later = {
        let (id, set) = (id.clone(), set.clone());
        move |f: ComponentFactory| {
            let current = with(|s| s.panels[area].as_ref().is_some_and(|(v, _)| *v == id));
            if current {
                with(|s| s.panels[area] = Some((id.clone(), f.clone())));
                set(f);
            }
        }
    };
    let f = factory(id, None, later);
    with(|s| s.panels[area] = Some((id.clone(), f.clone())));
    set(f);
}

/// Area `area` shows no panel of a plugin's now.
pub(crate) fn no_panel(area: usize) {
    with(|s| s.panels[area] = None);
}

/// The factory for a track section's view, made once for each opening of
/// the Track tab's sections (`dialog`); `resync` shows the sections again
/// once the plugin's file is compiled.
pub(crate) fn section(
    id: &ViewId,
    dialog: u64,
    track: TrackView,
    resync: impl FnOnce() + 'static,
) -> ComponentFactory {
    let key = (id.clone(), dialog);
    if let Some(f) = with(|s| s.sections.get(&key).cloned()) {
        return f;
    }
    let later_key = key.clone();
    let f = factory(id, Some(track), move |f| {
        with(|s| s.sections.insert(later_key, f));
        resync();
    });
    with(|s| {
        s.sections.retain(|(_, d), _| *d == dialog);
        s.sections.insert(key, f.clone());
    });
    f
}

/// The shell's id for a plugin's panel.
pub(crate) fn shell_id(id: &ViewId) -> String {
    format!("view:{}/{}", id.plugin, id.view)
}

/// The panel a shell id names, if it names one.
pub(crate) fn view_of(shell_id: &str) -> Option<ViewId> {
    let (plugin, view) = shell_id.strip_prefix("view:")?.split_once('/')?;
    Some(ViewId {
        plugin: plugin.to_owned(),
        view: view.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_cross_both_ways() {
        let v = ViewValue::List(vec![ViewValue::Struct(vec![
            ("name".into(), ViewValue::String("! Thing".into())),
            ("removable".into(), ViewValue::Bool(true)),
            ("size".into(), ViewValue::Number(2.5)),
        ])]);
        assert_eq!(from_slint(&to_slint(&v)), v);
        let image = ViewValue::Image {
            columns: 2,
            rows: 1,
            rgba: vec![1, 2, 3, 255, 4, 5, 6, 255],
        };
        assert_eq!(from_slint(&to_slint(&image)), image);
        assert_eq!(
            from_slint(&Value::EnumerationValue(
                "TextHorizontalAlignment".into(),
                "center".into()
            )),
            ViewValue::String("center".into())
        );
    }

    #[test]
    fn a_plugin_s_file_compiles_with_the_window_s_style_or_says_where_it_is_wrong() {
        let ok = compile(
            r#"import { Button } from "std-widgets.slint";
            export component Panel { in property <string> name; callback pressed(int);
                Button { text: name; clicked => { pressed(1); } } }"#
                .into(),
        )
        .unwrap();
        let def = ok.component("Panel").unwrap();
        assert_eq!(def.callbacks().collect::<Vec<_>>(), ["pressed"]);
        let wrong = compile("export component Panel { Nope { } }".into()).unwrap_err();
        assert!(wrong.starts_with("line 1, column"), "{wrong}");
    }

    #[test]
    fn panels_have_shell_ids_of_their_own() {
        let id = ViewId {
            plugin: "a.b".into(),
            view: "chat".into(),
        };
        assert_eq!(view_of(&shell_id(&id)), Some(id));
        assert_eq!(view_of("plugin:a.b/chat"), None);
    }
}
