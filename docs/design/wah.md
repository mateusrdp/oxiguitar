# Wah-wah playback (#250)

Status: built (core: this note's "As built"; the pedal: github.com/mateusrdp/oxiguitar-wah).

The issue: the palette's Closed and Open Wah-Wah marks were only drawn. A wah is a resonance swept by a pedal, and well documented; a mark should open a dialog with a curve, as a bend's, and a Q, with a Cry Baby's values where nothing is set.

## Decided by Mateus (15 Nov 2026)

- **The filter is an effect plugin of its own**, oxiguitar-wah, like the equalizer and the amp; the user orders it among a track's effects. So the interface grows: effects get what the score moves on their track (interface 1.8).
- **The pedal is off on beats without a wah mark.** It sounds only while a marked beat plays; a held position means a mark on each beat.
- **A mark's curve and Q are saved in the file beside the score** (`FILE.oxg`). The score file keeps the open or closed mark, as Guitar Pro reads it; nothing is lost on Save.

## The model

`BeatEffects::wah` stays the mark (`Wah::Open`, `Wah::Closed`), which is what the page draws ("o", "+") and what a `.gp` file holds. Beside it, `BeatEffects::wah_shape: Option<WahShape>`:

- `points`: the pedal across the beat, each point a fraction of the beat's duration (0 to 1) and a position from 0 (heel down, closed) to 100 (toe down, open), in time order; straight lines between them, the first and last held to the beat's ends.
- `q`: the filter's Q with the pedal half way, in tenths (10 to 160).

`None` is the kind's own shape, `Wah::shape`: the pedal rocks to its place over the beat's first half and stays there, from the heel for an open mark and from the toe for a closed one, with Q 4. So a lone open mark says "wah", and marks that alternate rock the pedal back and forth without a jump. `Command::Wah` sets a mark with a shape (or none) and keeps `None` when the shape is the kind's own; switching the mark with `Effect::Wah` drops the shape.

No score format holds a shape, so it does not cross the interface in the score (no `score2` extension): it lives in the sidecar, per track, as `"wah": [{ "at": [staff, bar, voice, beat], "q": 4.0, "points": [[0, 1, 0], [1, 2, 100], [1, 1, 100]] }]`, each point (numerator, denominator, position). Opening gives each listed beat its shape if it still has a wah mark, and ignores the entry otherwise (a file changed elsewhere, or saved in a format that drops the marks). The app and `oxiguitar-cli` (`play`, `export`) read it; `convert` copies the sidecar as before.

## Interface 1.8: control curves

An effect knew only its track's sound and settings. Since 1.8 the host tells it where the score is and hands it the track's **control curves**:

- `ControlPoint { time, value, control }` (C layout, `realtime.rs`): at `time` seconds of the score (100 % speed) the control is at `value`. A control's points are followed in straight lines; a point whose value is not a number ends a run, and the control has no value until its next point, nor before its first or after its last.
- `control::WAH` (the pedal, 0 to 1; no value means off) and `control::WAH_Q` (the mark's Q). An effect follows the controls it knows and ignores the rest, so later controls (a volume pedal, an expression curve) need no new shape.
- `ProcessCall` ends in `time`, `step` (the score's seconds per frame), `controls` and `control_count`: the track's whole array, sorted by control and then by time, the same while the song plays, as the pitch curves are for instruments. A plugin built for an older interface reads none of them.
- The SDK: `Processor::process_with(left, right, &Controls)`, which is `process` unless overridden; `Controls::time`, `step`, `points(control)` and `value_at(control, frame)`.

Why a curve handed over whole and not events: a seek, a loop or another speed needs nothing but the time; the effect keeps no state about the score; and the audio thread copies nothing.

The player: `events::controls(score, map)` makes each track's points from its wah marks in playing order (repeats played out, tempo changes inside a beat followed); two voices' marks at once follow the first. `Timeline::controls`, then `Schedule` keeps them per source and slot (a track's chord voice gets the same pedal), and the engine hands them to `Source::render` only while the song plays: stopped, paused or counting in, an effect gets none, so a note ringing on keeps no wah. `PluginSource` passes a track's points to each of its effects.

## The pedal (oxiguitar-wah)

Julius O. Smith's digital Cry Baby (CCRMA, "Making Virtual Electric Guitars and Associated Effects Using Faust", Digital CryBaby), our own code from its formulas: one resonance with a zero at 0 Hz, whose frequency, Q and gain follow the pedal `w`: f = 450 · 2^(2.3 w) Hz, Q = 2^(2 (1 − w) + 1), g = 0.1 · 4^w. The heel is a narrow peak at 450 Hz (Q 8), the toe a wide one at 2.2 kHz (Q 2). A mark's Q scales the pedal's own (4 is the Cry Baby), with the gain scaled the other way. The track section "Wah-wah" switches the pedal off for a track and sets its make (heel frequency, sweep in octaves; Cry Baby sets them back). The pedal comes in and goes out over 10 ms.

By default the wah runs first on a track, before the amp, as a pedal does (`DEFAULT_EFFECT_ORDER`: wah, amp, equalizer).

## The dialog

The palette's Closed and Open Wah-Wah open the **wah-wah editor** at the caret (or for the selection): the bend editor's dialog (`BendDialog`, now with its side labels, strong lines and words as properties) with the kind (No wah-wah, Open, Closed), the Q, and a grid of the beat across and the pedal from heel to toe up the side. On a beat without a mark it opens on the kind clicked, so Enter adds the plain mark; on a beat with one it shows the mark's shape. `wah_form.rs` holds its state and rules, as `bend_form.rs` does for bends. The Note tab still switches the plain marks.

## As built

- `oxiguitar-model`: `WahShape`, `WahPoint`, `Wah::shape`, `Track::wah_shapes`, `Track::shape_wah`.
- `oxiguitar-edit`: `Command::Wah`.
- `oxiguitar-host`: `sidecar::WahEntry`, `Instance::process` with the time and the controls; `DEFAULT_EFFECT_ORDER`.
- `oxiguitar-plugin-api` and `oxiguitar-plugin` 1.8.0: the above. The test plugin's `gain` follows the pedal as a volume pedal, and the contract suite rocks a pedal through every effect's real-time checks.
- `oxiguitar-play`: `events::controls`, `Timeline::controls`, `Schedule`, `engine::Controls`.
- `oxiguitar-app`: `wah_form.rs`, `Dialog::Wah`.

## Not done

- A wah on a MIDI file written by oxiguitar-midi-file (it could be a controller curve).
- The pedal's place when two voices disagree; a pedal held over beats without marks.
- Other makes as presets (a Vox's sweep is two sliders away).
