# Spikes

Small, throwaway experiments that settle a technical choice. Each spike ends in a decision record in [docs/adr/](../docs/adr/); the code stays here so the measurements can be repeated.

Spikes are **not** part of the main Cargo workspace: they have their own workspace and lock file, CI does not gate on them, and cargo-deny does not check their dependencies. Nothing in `crates/` may depend on a spike.

## gui/ — GUI-toolkit spike (M0, ADR 0004)

The same small screen in three toolkits: a palette of note-value and effect tools on the left, a track list along the bottom and the score canvas in the middle. The canvas is the real renderer (`oxiguitar-render`, ADR 0005) drawing the specimen page.

| Crate | Toolkit |
| --- | --- |
| `spike-slint` | Slint 1.18 (`slint/ui/app.slint` holds the screen) |
| `spike-iced` | iced 0.14 |
| `spike-egui` | egui and eframe 0.36 |
| `spike-common` | What all three share: tools, tracks, the canvas |

All three were built for the comparison in ADR 0004 and do the same things:

- Palette buttons with Bravura symbols, tooltips that name the tool and its shortcut, and selected states. Note values exclude each other; effects toggle.
- Single-key shortcuts (1–6 for note values, H, S, B, V, P, L, X, G, A, T for effects) that do nothing while you type in a text field.
- A search field that filters the palette.
- Track names you can edit (in Portuguese, Japanese and Arabic), mute and solo, a volume slider.
- The score canvas: zoom from 25 % to 400 %, a dark page, scrolling, and only the visible part of the page rendered.

The iced and egui spikes stay as they were for that comparison. **The Slint spike has since followed Mateus's review** (16 October 2026) and shows the rules in [docs/design/ui.md](../docs/design/ui.md):

- Only menus along the top (File, Edit, View, Help); no title, status caption or zoom buttons in the window. Most File and Edit items are placeholders, greyed out.
- View → Page: Light, Dark or Parchment. View → Palette (F7) and Tracks (F8) show or hide the panels; the splitters resize them.
- Palette groups collapse and expand from their headings. Ctrl + K focuses the tool search.
- Palette icons are drawn by our renderer and centred on their ink (`spike-common/src/icons.rs`; `cargo run -p spike-common --example icon_sheet -- icons.png` draws them all).
- Interface zoom: Ctrl + mouse wheel, Ctrl + `+` / `-` / `0`. Score zoom: the slider at the bottom right of the canvas; click its percentage for 100 %.
- Help → Keyboard and Mouse (F1) lists all of this.

### Running a spike

On Debian (trixie or sid) with the packages from the brief's GitHub setup tab installed:

```sh
cd spikes/gui
cargo run --release -p spike-slint   # or spike-iced, spike-egui
```

The first build of each takes several minutes. The binaries need no files beside them.
