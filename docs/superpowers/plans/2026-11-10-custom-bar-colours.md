# Custom Bar Colours Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let users assign colours to individual bars and sections, so staff lines, note heads, and symbols inherit that colour, with a four-level precedence system (Track Bar > Bar > Section > System).

**Architecture:** Two `Option<Color>` fields on `MasterBar`, a resolver method, sidecar persistence, two new editing commands, a colour picker in the bar dialog, and a colour parameter flowing through the engraving pipeline. Minimal surface area across five crates.

**Tech Stack:** Rust, serde_json (sidecar), Slint (UI), oxiguitar-model / oxiguitar-host / oxiguitar-edit / oxiguitar-app / oxiguitar-layout

**Spec:** `docs/superpowers/specs/2026-11-10-custom-bar-colours-design.md`

## Global Constraints

- `Color` struct exists in `oxiguitar-model/src/score.rs` with fields `r: u8, g: u8, b: u8`, derives `Clone, Copy, Debug, Default, PartialEq, Eq, Hash`
- `.oxg` sidecar is JSON, versioned `"oxiguitar": 1`, per-track, keyed by track name
- `MasterBar` is `Vec<MasterBar>` in `Score`, shared across all tracks
- `Command` enum is in `oxiguitar-edit/src/entry.rs`, variant match in `bars.rs` and `marks.rs` pattern
- `Dialog` enum is in `oxiguitar-app/src/editing.rs`, dialog components are in `bar_dialog.slint`
- Drawing uses `Painter::hline` for staff lines, `Painter::glyph` for symbols, in `oxiguitar-layout/src/engrave.rs`
- No polling; no new visual primitives (no background rectangles)
- Colours attached to bar index; survive bar insertions/deletions

## Review Focus

1. **Empty bar colour array after insert** — inserting a bar shifts indices; the sidecar array must shift too so colours stay with the right bar
2. **Section colour default (0,0,0) vs explicit black** — `Color::default()` is treated as "no colour"; a user who explicitly sets a section to black falls through (low risk, matches existing convention)
3. **Track name mismatch after rename** — `track_bar_colors` keyed by track name; renaming a track must update the key (same pattern as existing plugin settings)
4. **Colour resolution when all levels are None** — falls back to `system_colour`; the engraver must always have a valid default
5. **Sidecar read failure graceful degradation** — if `.oxg` is corrupt, colours start empty; the app must not crash

---

## Task 1: Model — Add colour fields to `MasterBar`

**Files:**
- Modify: `crates/oxiguitar-model/src/score.rs`

**Interfaces:**
- Consumes: `Color` struct (already exists in this file)
- Produces: `MasterBar::bar_color: Option<Color>`, `MasterBar::track_bar_color: Option<Color>`, `MasterBar::resolve_colour(&self, track_bar_color: Option<Color>, system_colour: Color) -> Color`

- [ ] **Step 1: Add two fields to `MasterBar` struct**

Add at the end of the struct (before the closing `}`), after `fermatas`:

```rust
    /// Custom colour for this bar (global: all tracks). `None` for no custom colour.
    pub bar_color: Option<Color>,
    /// Custom colour for this bar on the selected track only. `None` for no custom colour.
    pub track_bar_color: Option<Color>,
```

- [ ] **Step 2: Update `Default` impl for `MasterBar`**

Add to the `Default` impl's `..Default::default()` block (or add the fields explicitly):

```rust
            bar_color: None,
            track_bar_color: None,
```

- [ ] **Step 3: Add `resolve_colour` method to `impl MasterBar`**

Add after the existing `expected_length` method:

```rust
    /// Resolve the effective colour for this bar by precedence:
    /// Track Bar Colour > Bar Colour > Section Colour > System Colour.
    ///
    /// Section colour is taken from `marker.color` when it is not
    /// `Color::default()` (0, 0, 0), which means "no section colour set".
    pub fn resolve_colour(
        &self,
        track_bar_color: Option<Color>,
        system_colour: Color,
    ) -> Color {
        track_bar_color
            .or(self.bar_color)
            .or(self.marker.as_ref().and_then(|m| {
                if m.color != Color::default() {
                    Some(m.color)
                } else {
                    None
                }
            }))
            .unwrap_or(system_colour)
    }
```

- [ ] **Step 4: Run model tests**

Run: `cargo test -p oxiguitar-model --lib`
Expected: PASS (no existing tests should break; the new fields have defaults)

- [ ] **Step 5: Commit**

```bash
git add crates/oxiguitar-model/src/score.rs
git commit -m "feat(model): add bar colour fields and resolver to MasterBar"
```

---

## Task 2: Persistence — Extend `.oxg` sidecar with bar colours

**Files:**
- Modify: `crates/oxiguitar-host/src/sidecar.rs`

**Interfaces:**
- Consumes: `oxiguitar_model::Color` (via `oxiguitar-model` dependency), existing `Sidecar` struct
- Produces: `Sidecar::bar_colors: Vec<Option<serde_json::Value>>`, `Sidecar::track_bar_colors: BTreeMap<String, Vec<Option<serde_json::Value>>>`, parse/write round-trip

- [ ] **Step 1: Add colour fields to `Sidecar` struct**

Add to the `Sidecar` struct (after `tracks`):

```rust
    /// One entry per bar: `None` for no custom colour, or an RGB object.
    pub bar_colors: Vec<Option<serde_json::Value>>,
    /// Track name → one entry per bar. Same structure as `bar_colors`.
    pub track_bar_colors: BTreeMap<String, Vec<Option<serde_json::Value>>>,
```

- [ ] **Step 2: Update `is_empty` to consider colour fields**

Change the `is_empty` method to also check if colour arrays are empty or all-null:

```rust
    pub fn is_empty(&self) -> bool {
        self.tracks
            .iter()
            .all(|t| t.plugins.is_empty() && t.effects.is_empty() && !t.chords)
            && self.bar_colors.iter().all(Option::is_none)
            && self.track_bar_colors.values().all(|v| v.iter().all(Option::is_none))
    }
```

- [ ] **Step 3: Update `parse` to read colour fields**

In the `parse` method, after reading `"tracks"`, add reading of the new fields:

```rust
            "bar_colors" => {
                let arr = value.as_array().ok_or("bar_colors not an array")?;
                sidecar.bar_colors = arr
                    .iter()
                    .map(|v| if v.is_null() { None } else { Some(v.clone()) })
                    .collect();
            }
            "track_bar_colors" => {
                let obj = value.as_object().ok_or("track_bar_colors not an object")?;
                sidecar.track_bar_colors = obj
                    .iter()
                    .map(|(k, v)| {
                        let arr = v.as_array().unwrap_or(&Vec::new());
                        (
                            k.clone(),
                            arr.iter()
                                .map(|item| if item.is_null() { None } else { Some(item.clone()) })
                                .collect(),
                        )
                    })
                    .collect(),
            }
```

- [ ] **Step 4: Update `format` to write colour fields**

In the `format` method, after building `"tracks"`, add the new fields to the JSON object:

```rust
        if !self.bar_colors.iter().all(Option::is_none) {
            let arr: Vec<Value> = self
                .bar_colors
                .iter()
                .map(|c| c.clone().unwrap_or(Value::Null))
                .collect();
            doc["bar_colors"] = serde_json::json!(arr);
        }
        if !self.track_bar_colors.is_empty() {
            let obj: Map<String, Value> = self
                .track_bar_colors
                .iter()
                .map(|(name, colors)| {
                    let arr: Vec<Value> = colors
                        .iter()
                        .map(|c| c.clone().unwrap_or(Value::Null))
                        .collect();
                    (name.clone(), serde_json::json!(arr))
                })
                .collect();
            doc["track_bar_colors"] = Value::Object(obj);
        }
```

- [ ] **Step 5: Run sidecar tests**

Run: `cargo test -p oxiguitar-host --lib sidecar`
Expected: PASS (existing sidecar tests should still pass; the new fields are optional in parse)

- [ ] **Step 6: Commit**

```bash
git add crates/oxiguitar-host/src/sidecar.rs
git commit -m "feat(host): extend sidecar with bar colour persistence"
```

---

## Task 3: Editing — Add bar colour commands

**Files:**
- Modify: `crates/oxiguitar-edit/src/entry.rs`
- Modify: `crates/oxiguitar-edit/src/bars.rs`

**Interfaces:**
- Consumes: `oxiguitar_model::Color`, existing `MasterBar` fields
- Produces: `Command::SetBarColor(Option<Color>)`, `Command::SetTrackBarColor(Option<Color>)`, handling in `bars.rs::apply` and `bars.rs::master`

- [ ] **Step 1: Add two command variants to `Command` enum**

Add at the end of the enum (before the closing `}`), after `SystemLayout`:

```rust
    /// This colour for the caret's bar (global: all tracks); `None` for none.
    SetBarColor(Option<Color>),
    /// This colour for the caret's bar on the selected track only; `None` for none.
    SetTrackBarColor(Option<Color>),
```

- [ ] **Step 2: Add label entries for the new commands**

In `Command::label()`, add after the existing `match` arms:

```rust
            Command::SetBarColor(_) => "Bar Colour",
            Command::SetTrackBarColor(_) => "Track Bar Colour",
```

- [ ] **Step 3: Add command handling in `bars.rs::apply`**

In the `match command` block of `apply()`, add before the `_` catch-all:

```rust
        Command::SetBarColor(color) => {
            one(first, &|b| b.bar_color = color)
        }
        Command::SetTrackBarColor(color) => {
            one(first, &|b| b.track_bar_color = color)
        }
```

- [ ] **Step 4: Run editing tests**

Run: `cargo test -p oxiguitar-edit --lib`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/oxiguitar-edit/src/entry.rs crates/oxiguitar-edit/src/bars.rs
git commit -m "feat(edit): add SetBarColor and SetTrackBarColor commands"
```

---

## Task 4: UI — Colour picker dialog and wiring

**Files:**
- Modify: `crates/oxiguitar-app/ui/bar_dialog.slint`
- Modify: `crates/oxiguitar-app/src/editing.rs`
- Modify: `crates/oxiguitar-app/src/bar_form.rs`
- Modify: `crates/oxiguitar-app/src/main.rs`

**Interfaces:**
- Consumes: `oxiguitar_model::Color`, `Command` variants from Task 3, `Sidecar` from Task 2
- Produces: `DisplayOptionsDialog` component in Slint, `Dialog::DisplayOptions` variant, right-click menu entry

- [ ] **Step 1: Add `DisplayOptionsDialog` component to `bar_dialog.slint`**

Add at the end of the file (before the closing `}`):

```slint
// Display Options… (bar colours): pick a bar colour or track bar colour.
export component DisplayOptionsDialog inherits Rectangle {
    in-out property <bool> bar-colour-set: false;
    in-out property <int> bar-colour-r: 0;
    in-out property <int> bar-colour-g: 0;
    in-out property <int> bar-colour-b: 0;
    in-out property <bool> track-bar-colour-set: false;
    in-out property <int> track-bar-colour-r: 0;
    in-out property <int> track-bar-colour-g: 0;
    in-out property <int> track-bar-colour-b: 0;
    callback done(bool, bool, int, int, int, bool, int, int, int);
    Card {
        width: root.width;
        height: root.height;
        heading: "Display Options";
        card-width: 420px;
        done(ok) => {
            root.done(ok,
                root.bar-colour-set, root.bar-colour-r, root.bar-colour-g, root.bar-colour-b,
                root.track-bar-colour-set, root.track-bar-colour-r, root.track-bar-colour-g, root.track-bar-colour-b);
        }
        VerticalLayout {
            spacing: 8px;
            // Bar Colour (global)
            HorizontalLayout {
                spacing: 8px;
                Label { text: "Bar Colour"; }
                CheckBox {
                    text: root.bar-colour-set ? "" : "Default";
                    checked: root.bar-colour-set;
                    toggled => { root.bar-colour-set = self.checked; }
                }
                Rectangle {
                    width: 24px;
                    height: 24px;
                    border-radius: 4px;
                    background: rgb(root.bar-colour-set ? root.bar-colour-r : 128,
                                    root.bar-colour-set ? root.bar-colour-g : 128,
                                    root.bar-colour-set ? root.bar-colour-b : 128);
                    TouchArea {
                        clicked => { /* open system colour picker — placeholder */ }
                    }
                }
                Text { text: root.bar-colour-set ? "Custom" : "Default"; font-size: 11px; opacity: 0.6; vertical-alignment: center; }
            }
            // Track Bar Colour (per-track)
            HorizontalLayout {
                spacing: 8px;
                Label { text: "Track Bar Colour"; }
                CheckBox {
                    text: root.track-bar-colour-set ? "" : "Default";
                    checked: root.track-bar-colour-set;
                    toggled => { root.track-bar-colour-set = self.checked; }
                }
                Rectangle {
                    width: 24px;
                    height: 24px;
                    border-radius: 4px;
                    background: rgb(root.track-bar-colour-set ? root.track-bar-colour-r : 128,
                                    root.track-bar-colour-set ? root.track-bar-colour-g : 128,
                                    root.track-bar-colour-set ? root.track-bar-colour-b : 128);
                    TouchArea {
                        clicked => { /* open system colour picker — placeholder */ }
                    }
                }
                Text { text: root.track-bar-colour-set ? "Custom" : "Default"; font-size: 11px; opacity: 0.6; vertical-alignment: center; }
            }
            Text {
                text: "Section colour is set via the Bar tab's section marker. Colours cascade: Track Bar > Bar > Section > System.";
                font-size: 11px;
                wrap: word-wrap;
                color: Palette.foreground.transparentize(0.4);
            }
        }
    }
}
```

**Note:** Slint has no built-in colour picker. The `TouchArea` on the colour swatch is a placeholder. For the first implementation, the user can type RGB values via three `SpinBox` widgets instead of a system colour picker. The Swatch shows the current colour. This keeps the scope contained.

Let me revise Step 1 to use spinboxes for RGB instead of a system colour picker:

```slint
// Display Options… (bar colours): pick a bar colour or track bar colour.
export component DisplayOptionsDialog inherits Rectangle {
    in-out property <bool> bar-colour-set: false;
    in-out property <int> bar-colour-r: 128;
    in-out property <int> bar-colour-g: 128;
    in-out property <int> bar-colour-b: 128;
    in-out property <bool> track-bar-colour-set: false;
    in-out property <int> track-bar-colour-r: 128;
    in-out property <int> track-bar-colour-g: 128;
    in-out property <int> track-bar-colour-b: 128;
    callback done(bool, bool, int, int, int, bool, int, int, int);
    Card {
        width: root.width;
        height: root.height;
        heading: "Display Options";
        card-width: 420px;
        done(ok) => {
            root.done(ok,
                root.bar-colour-set, root.bar-colour-r, root.bar-colour-g, root.bar-colour-b,
                root.track-bar-colour-set, root.track-bar-colour-r, root.track-bar-colour-g, root.track-bar-colour-b);
        }
        VerticalLayout {
            spacing: 8px;
            // Bar Colour (global)
            Label { text: "Bar Colour"; }
            HorizontalLayout {
                spacing: 8px;
                CheckBox {
                    text: "Enable";
                    checked: root.bar-colour-set;
                    toggled => { root.bar-colour-set = self.checked; }
                }
                Rectangle {
                    width: 20px;
                    height: 20px;
                    border-radius: 3px;
                    background: rgb(root.bar-colour-set ? root.bar-colour-r : 128,
                                    root.bar-colour-set ? root.bar-colour-g : 128,
                                    root.bar-colour-set ? root.bar-colour-b : 128);
                }
                Text { text: "R"; font-size: 11px; vertical-alignment: center; }
                SpinBox { minimum: 0; maximum: 255; value: root.bar-colour-set ? root.bar-colour-r : 128; enabled: root.bar-colour-set; edited(v) => { root.bar-colour-r = v; } }
                Text { text: "G"; font-size: 11px; vertical-alignment: center; }
                SpinBox { minimum: 0; maximum: 255; value: root.bar-colour-set ? root.bar-colour-g : 128; enabled: root.bar-colour-set; edited(v) => { root.bar-colour-g = v; } }
                Text { text: "B"; font-size: 11px; vertical-alignment: center; }
                SpinBox { minimum: 0; maximum: 255; value: root.bar-colour-set ? root.bar-colour-b : 128; enabled: root.bar-colour-set; edited(v) => { root.bar-colour-b = v; } }
            }
            // Track Bar Colour (per-track)
            Label { text: "Track Bar Colour"; }
            HorizontalLayout {
                spacing: 8px;
                CheckBox {
                    text: "Enable";
                    checked: root.track-bar-colour-set;
                    toggled => { root.track-bar-colour-set = self.checked; }
                }
                Rectangle {
                    width: 20px;
                    height: 20px;
                    border-radius: 3px;
                    background: rgb(root.track-bar-colour-set ? root.track-bar-colour-r : 128,
                                    root.track-bar-colour-set ? root.track-bar-colour-g : 128,
                                    root.track-bar-colour-set ? root.track-bar-colour-b : 128);
                }
                Text { text: "R"; font-size: 11px; vertical-alignment: center; }
                SpinBox { minimum: 0; maximum: 255; value: root.track-bar-colour-set ? root.track-bar-colour-r : 128; enabled: root.track-bar-colour-set; edited(v) => { root.track-bar-colour-r = v; } }
                Text { text: "G"; font-size: 11px; vertical-alignment: center; }
                SpinBox { minimum: 0; maximum: 255; value: root.track-bar-colour-set ? root.track-bar-colour-g : 128; enabled: root.track-bar-colour-set; edited(v) => { root.track-bar-colour-g = v; } }
                Text { text: "B"; font-size: 11px; vertical-alignment: center; }
                SpinBox { minimum: 0; maximum: 255; value: root.track-bar-colour-set ? root.track-bar-colour-b : 128; enabled: root.track-bar-colour-set; edited(v) => { root.track-bar-colour-b = v; } }
            }
            Text {
                text: "Section colour is set via the Bar tab's section marker. Colours cascade: Track Bar > Bar > Section > System.";
                font-size: 11px;
                wrap: word-wrap;
                color: Palette.foreground.transparentize(0.4);
            }
        }
    }
}
```

- [ ] **Step 2: Add `DisplayOptions` variant to `Dialog` enum**

In `editing.rs`, add to the `Dialog` enum:

```rust
    /// Display Options… (bar colours).
    DisplayOptions,
```

- [ ] **Step 3: Wire the dialog open in `main.rs`**

In the page menu (right-click) handler, add a "Display Options…" entry. In `main.rs`, find where the menu is built and add:

```rust
// In the menu items list:
MenuItem { text: "Display Options…"; triggered => { open_edit_dialog(&a, editing::Dialog::DisplayOptions); } }
```

- [ ] **Step 4: Handle the dialog in `open_edit_dialog`**

In `editing.rs`'s `open_edit_dialog` function, add a case for `Dialog::DisplayOptions`:

```rust
Dialog::DisplayOptions => {
    // Show DisplayOptionsDialog, read values, send SetBarColor / SetTrackBarColor commands
}
```

The handler reads the caret's bar index, the current `bar_color` and `track_bar_color` from `MasterBar`, populates the dialog, and on OK sends the appropriate `Command::SetBarColor` or `Command::SetTrackBarColor`.

- [ ] **Step 5: Run app build**

Run: `cargo check -p oxiguitar-app`
Expected: PASS (no compile errors)

- [ ] **Step 6: Commit**

```bash
git add crates/oxiguitar-app/ui/bar_dialog.slint crates/oxiguitar-app/src/editing.rs crates/oxiguitar-app/src/main.rs
git commit -m "feat(app): add Display Options dialog with RGB colour pickers"
```

---

## Task 5: Layout — Pass resolved colour to engraver and draw functions

**Files:**
- Modify: `crates/oxiguitar-layout/src/engrave.rs`
- Potentially modify: `crates/oxiguitar-layout/src/draw.rs` (if `hline`/`glyph` colour parameters need extending)

**Interfaces:**
- Consumes: `MasterBar::resolve_colour()`, `Color` from model
- Produces: resolved colour passed to `staff_lines()` and note-drawing functions

- [ ] **Step 1: Identify the draw calls that need the colour**

In `engrave.rs`, find all calls to `self.p.hline()` for staff lines and note-head drawing. These are the calls that need to receive the resolved colour instead of the default `Paint::Ink`.

- [ ] **Step 2: Thread the colour through the engraving pipeline**

The `Engraver` struct holds per-bar state. Add the resolved colour as a field on the bar's engraving context, or pass it as a parameter to the drawing functions.

In the bar drawing loop (where `staff_lines()` is called), resolve the colour:

```rust
let colour = master_bar.resolve_colour(
    track_bar_color,  // from the track's selection state
    system_colour,    // the default colour from the palette
);
```

- [ ] **Step 3: Update `staff_lines` to accept a colour parameter**

Change the signature:

```rust
fn staff_lines(&self, list: &mut DisplayList, left: f32, right: f32, colour: Color) {
```

Update the `hline` calls inside to use the colour for the paint parameter.

- [ ] **Step 4: Update note-head and symbol drawing**

Similarly update the note-head drawing calls to use the resolved colour. Note heads are drawn as filled circles — the colour parameter controls the fill.

- [ ] **Step 5: Run layout tests**

Run: `cargo test -p oxiguitar-layout --lib`
Expected: PASS

- [ ] **Step 6: Run full integration check**

Run: `cargo check --workspace`
Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add crates/oxiguitar-layout/src/engrave.rs crates/oxiguitar-layout/src/draw.rs
git commit -m "feat(layout): pass resolved bar colour to staff lines and note symbols"
```

---

## Task 6: Integration — Wire sidecar save/load with colour changes

**Files:**
- Modify: `crates/oxiguitar-app/src/document.rs` (save/load integration)

**Interfaces:**
- Consumes: `Sidecar` from Task 2, `MasterBar` fields from Task 1
- Produces: bar colours saved on score save, loaded on score open

- [ ] **Step 1: On score load, read sidecar bar colours into `MasterBar`**

In the document's load path, after reading the sidecar, iterate `bar_colors` and set `MasterBar.bar_color` for each bar index. Do the same for `track_bar_colors` keyed by track name.

- [ ] **Step 2: On score save, write `MasterBar` colours to sidecar**

In the document's save path, before writing the sidecar, populate `Sidecar.bar_colors` and `Sidecar.track_bar_colors` from the current `MasterBar` values.

- [ ] **Step 3: Run full workspace build**

Run: `cargo check --workspace`
Expected: PASS

- [ ] **Step 4: Run all tests**

Run: `cargo test --workspace`
Expected: PASS (or known failures documented)

- [ ] **Step 5: Commit**

```bash
git add crates/oxiguitar-app/src/document.rs
git commit -m "feat(app): wire bar colour persistence to sidecar save/load"
```

---

## Verification

After all tasks are complete:

1. **Manual test:** Open a `.gp` file, right-click a bar, set a bar colour via Display Options, verify the staff lines and note heads change colour.
2. **Precedence test:** Set a section colour, then override a bar within it with a bar colour, verify the bar colour takes precedence.
3. **Track test:** Set a track bar colour on a selected track, switch to another track, verify the colour doesn't appear.
4. **Persistence test:** Set colours, save, close, reopen — verify colours are restored.
5. **Insert test:** Insert a bar before a coloured bar — verify the colour stays with the original bar (index shifts in sidecar).
