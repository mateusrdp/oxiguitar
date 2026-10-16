# Architecture

OxiGuitar is a Cargo workspace. One score model sits at the centre; readers and writers, layout, playback and the GUI are separate crates around it. The GUI can change without touching the music code, and a command-line tool can convert, render and test files in CI without a window.

```text
                 oxiguitar-app (GUI shell, Slint)   oxiguitar-cli (no window)
                     |        |        \              |        |
                     v        v         v             v        v
          oxiguitar-render  oxiguitar-play   oxiguitar-edit  oxiguitar-formats
          (pixels, PDF)     (events, synth)  (commands,      (detect, read,
                |                 |           undo/redo)      write, import log)
                v                 |               |               |
          oxiguitar-display <- oxiguitar-layout   |               |
          (display list,       (engraving)        |               |
           fonts, SMuFL)           |              |               |
                                   \______________|_______________/
                                                  v
                                           oxiguitar-model
                              Score > Track > Staff > Bar > Voice > Beat > Note
```

Arrows show which crate calls which. Every crate reads or writes the shared model, so a new file format or output touches one box.

## Crates

| Crate | Status | Purpose |
| --- | --- | --- |
| `oxiguitar-model` | M0 | The score model: data types only, no I/O. |
| `oxiguitar-formats` | M0 | Format detection from contents (FMT-6), readers and writers, import logs (FMT-2). |
| `oxiguitar-cli` | M0 | `info`, later `convert`, `render` and test helpers. Used by CI. |
| `oxiguitar-edit` | M2 | Edit commands, selection, undo and redo. |
| `oxiguitar-display` | M0 | The display list (positioned glyph runs, lines, rectangles, paths, colour roles), the bundled fonts and SMuFL metadata (ADR 0005). |
| `oxiguitar-layout` | M1 | Engraving: turns the model into a display list. |
| `oxiguitar-render` | M0 | Draws a display list into pixels (screen, PNG) with tiny-skia and into PDF with krilla (ADR 0005); SVG later. |
| `oxiguitar-play` | M3 | Score-to-event conversion, the synthesiser, the real-time audio thread. |
| `oxiguitar-app` | M1 | The desktop GUI shell, in Slint (ADR 0004). Shows the renderer's pixels as the score canvas. |

Crates marked with a later milestone do not exist yet. Throwaway experiments live in [spikes/](spikes/), outside the workspace.

## Design rules

- **One model.** Every format is read into, and written from, the same score model: Score, Track, Staff, Bar, Voice, Beat, Note, with effects attached to the beat or note they act on. Format-specific data we cannot represent goes into the import log, never silently away.
- **Exact time.** Durations and positions are exact fractions, so tuplets, nested tuplets and grace notes never drift through rounding.
- **Edits are commands.** Every change is a reversible command object, which gives unlimited undo and redo.
- **Layout produces a display list.** The engraving engine outputs positioned glyphs and lines. Screen, PDF, SVG and PNG all draw the same list, so what you see is what you export.
- **Real-time audio never waits.** The audio thread allocates no memory and takes no locks; it receives events through lock-free queues.
- **Small core, optional plugins.** From M5 on, the metronome, NAM, RSE and anything needing special hardware are plugins.
- **Thin GUI.** All logic lives in toolkit-free crates, because Rust GUI toolkits change fast.
- **Untrusted input.** Files come from anywhere. Readers never panic, never allocate without bounds checks, and report errors with the file and position.
- **No network** except downloads the user starts (NET-1, NET-2). CI enforces this.

## Where decisions are recorded

[docs/adr/](docs/adr/) holds one short record per significant decision. [docs/formats/](docs/formats/) holds what we have learned about each file format.
