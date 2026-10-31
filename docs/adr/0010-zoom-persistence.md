# 0010 — Persist score and interface zoom across restarts

- Status: accepted
- Date: 2026-10-31
- Milestone: M2

## Context

Issue #147: both score zoom and interface zoom reset to their defaults (100 %)
every time the app restarts. Users who spend time zoomed in on a difficult
section or at a comfortable interface size lose that context on every launch.

Both zooms are already stored in the `Shared` struct (`zoom: f32` for score
zoom, `ui_zoom: f32` for interface zoom). The View menu's other settings are
persisted through `ViewPrefs` in `prefs.rs`, written to `settings.json` under
the `"view"` key.

## Options

1. **Store both zooms in `ViewPrefs`** alongside the existing View menu
   settings. Pros: single persistence path, one settings file, follows the
   existing pattern. Cons: none.

2. **Store zooms separately** (e.g. in a `"zoom"` key). Pros: keeps zooms
   separate from View menu settings. Cons: two places to read, two places to
   write, breaks the "all View settings together" mental model.

3. **Store zooms in window state** (position, size, scale). Pros: zoom is
   window-related. Cons: the app doesn't persist window state through Slint,
   and the scale factor is already influenced by system DPI — mixing user
   zoom with window state is messy.

## Decision

Option 1. `ViewPrefs` gains two fields: `score_zoom: f32` and `ui_zoom: f32`,
both defaulting to `1.0`. They are read in `apply_prefs()` and written in
`settings_changed()` plus the zoom change handlers (`set_zoom` for score zoom,
the `Ctrl`/`Alt` key handler for interface zoom).

## Consequences

- The user's zoom is preserved across restarts. Score zoom and interface zoom
  are independent (changing one does not affect the other).
- `ViewPrefs` now contains `f32` fields, which do not implement `Eq`. The
  derive was replaced with a manual `Eq` impl that delegates to `PartialEq`
  (the values are never NaN).
- Score zoom is persisted on every change (slider move, wheel zoom, preset
  key). Interface zoom is persisted on every key press that changes it. Both
  are also persisted when `settings_changed()` fires (vertical layout zoom,
  app close).
- The default for both zooms is `1.0` (100 %). Existing users without a
  persisted zoom get the default, which is the current behaviour.
