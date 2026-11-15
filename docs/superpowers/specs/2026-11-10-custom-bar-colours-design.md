# Custom Bar Colours — Design Doc

**Issue:** #269
**Date:** 2026-11-10
**Status:** Draft — awaiting approval

## Intent

Let users assign colours to individual bars and sections in the tablature viewer, so that staff lines, note heads, and symbols inherit that colour. This makes coloured sections and highlighted bars visually clear without the noise of full-background highlighting.

## Goals

- Assign colours to bars and sections via a right-click "Display Options" dialog
- Four colour levels with strict precedence: **Track Bar Colour > Bar Colour > Section Colour > System Colour**
- Colours cascade: if a bar colour is cleared, it falls back to the next level down
- Colours persist in the `.oxg` sidecar file (extended for this purpose)
- Colours survive bar insertions and deletions (attached to bar index, not bar content)
- Minimal code changes: two new fields on `MasterBar`, one resolver function, one engraving tweak

## Non-Goals

- Bar colours in external formats (Guitar Pro, MusicXML, etc.) — these formats don't support it; colours live only in `.oxg`
- Full bar background highlighting (the issue originally proposed this; user requested lines/symbols only)
- Per-voice or per-beat colours — bar-level only
- Colour presets, palettes, or themes

## Anti-Goals

- Losing colours when bars are inserted/deleted (colours must stay with the bar at its index)
- Colours bleeding across tracks when they shouldn't (Track Bar Colour is per-track, Bar Colour is global)
- New visual primitives in the layout engine (no background rectangles; reuse existing line/symbol drawing)

## Constraints

- `Color` struct already exists in `oxiguitar-model/src/score.rs` (RGB, `Clone, Copy, Debug, Default, PartialEq, Eq, Hash`)
- `MasterBar` is the shared bar structure across all tracks — the right place for global bar colours
- `.oxg` sidecar format is JSON, versioned (`"oxiguitar": 1`), per-track today — extend with a new top-level field
- Existing section markers (`Marker`) already carry a `color: Color` field — bar colours are a separate concern on the same struct
- Drawing uses `Painter` primitives in `oxiguitar-layout/src/draw.rs` and `engrave.rs` — colour is passed as a parameter to line/symbol drawing calls

## Approaches

### Approach A: Colour fields on `MasterBar` (Recommended)

Add `bar_color: Option<Color>` and `track_bar_color: Option<Color>` to `MasterBar`. A resolver function (`MasterBar::resolve_colour`) picks the effective colour by precedence. The engraver passes this colour to the drawing calls for staff lines and note symbols.

**Pros:** Minimal surface area. `MasterBar` already exists. Two `Option<Color>` fields. One resolver method. Drawing code swaps one colour for another. `.oxg` sidecar extends naturally.

**Cons:** Bar colours share a struct with section markers — but they're orthogonal fields (`Option<Color>`) and this pattern already exists (e.g. `marker: Option<Marker>`).

### Approach B: Sidecar-only colour storage

Store bar colours only in the `.oxg` sidecar, not in the model. The app reads them from the sidecar and resolves at draw time.

**Pros:** No model changes.

**Cons:** Colours are detached from the score model. Undo/redo would need separate handling. More complex wiring between sidecar and drawing.

### Why A over B

Approach A keeps colours in the model where undo/redo already operates. The sidecar becomes a persistence layer, not the source of truth. This is simpler end-to-end.

## How It Works

### Data Model

**`MasterBar` gains two fields:**

```rust
pub struct MasterBar {
    // ... existing fields ...
    pub bar_color: Option<Color>,       // Global bar colour (all tracks)
    pub track_bar_color: Option<Color>, // Per-track bar colour (only on selected track)
}
```

**`.oxg` sidecar extension** (version stays 1, new field):

```json
{
  "oxiguitar": 1,
  "tracks": [...],
  "bar_colors": [
    null, null, {"r": 255, "g": 100, "b": 100}, null, ...
  ],
  "track_bar_colors": {
    "Lead Guitar": [null, null, {"r": 100, "g": 100, "b": 255}, null, ...],
    "Bass": [null, null, null, null, ...]
  }
}
```

- `bar_colors`: one entry per bar (index = bar position). `null` = no custom colour.
- `track_bar_colors`: keyed by track name (same as existing sidecar pattern for track matching). Each value is one entry per bar.

### Colour Resolution

```rust
impl MasterBar {
    pub fn resolve_colour(
        &self,
        track_bar_color: Option<Color>,
        system_colour: Color,
    ) -> Color {
        // Track Bar Colour > Bar Colour > Section Colour > System Colour
        track_bar_color
            .or(self.bar_color)
            .or(self.marker.as_ref().and_then(|m| {
                if m.color != Color::default() { Some(m.color) } else { None }
            }))
            .unwrap_or(system_colour)
    }
}
```

Note: Section colour (`marker.color`) is the existing section marker colour. A `Color::default()` (0, 0, 0) means "no section colour set" — the resolver skips it and falls through.

### UI

**Right-click on a bar** → opens or extends the existing "Display Options" dialog (in `bar_dialog.slint`) with:

- **Bar Colour** picker (global, all tracks)
  - Shows current colour or "Default"
  - Toggling "Default" clears the colour, falling back to the next level
- **Track Bar Colour** picker (per-track, only visible when that track is selected)
  - Shows current colour or "Default"
  - Toggling "Default" clears the colour, falling back to the next level
- Section colour is **read-only** (set via the existing section marker dialog)

**Dialog flow:**
1. User right-clicks a bar
2. Dialog shows current colour state for both bar colour and track bar colour
3. User picks a colour or toggles to "Default"
4. Change applies immediately (like other bar dialogs)
5. Change is recorded as an undoable command

### Persistence

- On save: the app writes `bar_colors` and `track_bar_colors` to the `.oxg` sidecar
- On load: the app reads these fields and populates `MasterBar.bar_color` / `track_bar_color`
- If the sidecar is missing or can't be read: colours start empty (all bars use section/system colours)
- `oxiguitar-cli convert` copies the `.oxg` sidecar as usual (existing behaviour)

### Drawing

The engraver (`oxiguitar-layout/src/engrave.rs`) currently draws staff lines and note symbols with a fixed or context-derived colour. The change:

1. The layout engine computes the effective colour for each bar using `resolve_colour()`
2. The colour is passed through the plan/engrave pipeline as a bar-level parameter
3. Staff line drawing (`staff_lines()`) and note/symbol drawing use the resolved colour instead of the default

**What changes colour:**
- Staff lines (the 6 tab lines, or 5 standard staff lines)
- Note heads (filled/empty circles)
- Note stems and beams
- Bar symbols (repeat signs, fermatas, etc. that are bar-specific)

**What does NOT change colour:**
- Beat-level effects (slurs, vibrato, tremolo, etc.) — these are not bar-specific
- Section markers (they already have their own colour)
- Track names, bar numbers, time/key signatures

### Editing

Bar colour changes are recorded as editing commands in `oxiguitar-edit/src/bars.rs`:

```rust
// New command variants
SetBarColor { bar_index: usize, color: Option<Color> }
SetTrackBarColor { bar_index: usize, color: Option<Color> }
```

These follow the existing pattern (see `marks.rs` for how mark commands are structured).

## Files Changed

| File | Change |
|------|--------|
| `crates/oxiguitar-model/src/score.rs` | Add `bar_color` and `track_bar_color` to `MasterBar`, add `resolve_colour()` method, update `Default` impl |
| `crates/oxiguitar-host/src/sidecar.rs` | Extend `Sidecar` struct with `bar_colors` and `track_bar_colors` fields, update parse/write |
| `crates/oxiguitar-edit/src/bars.rs` | Add `SetBarColor` and `SetTrackBarColor` commands |
| `crates/oxiguitar-app/ui/bar_dialog.slint` | Add colour picker UI to the bar dialog |
| `crates/oxiguitar-app/src/bar_form.rs` | Wire dialog to command execution |
| `crates/oxiguitar-layout/src/engrave.rs` | Pass resolved colour to line/symbol drawing |
| `crates/oxiguitar-layout/src/draw.rs` | Accept colour parameter in relevant draw functions |

## Builder Calls (My Decisions)

The following are my calls as the builder, not requiring user approval:

1. **Section colour fallback:** `Color::default()` (0, 0, 0) is treated as "no colour set" in the resolver. If a user explicitly sets a section marker to black, it would fall through — but this matches the existing convention (section colours are set via a dialog that starts with a non-black default).

2. **`.oxg` version:** Stays at 1. The new fields are optional and ignored by older OxiGuitar versions.

3. **Track name matching for `track_bar_colors`:** Same pattern as existing sidecar track matching — by name, not index. A track that moved position finds its colours by name.

4. **Dialog integration:** Extends the existing bar dialog rather than creating a new one. The "Display Options" entry on right-click opens this dialog.

5. **Default system colour:** The existing default colour for tabs (typically a grey or light colour) is used as the system colour when no custom colour is set at any level.

## Open Questions

None — all decisions made.
