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
- View → Dark Page. View → Layout lists the four staff layouts (Page, Vertical, Horizontal, Parchment), greyed until M1. View → Palette (F7) and Tracks (F8) show or hide the panels; the splitters resize them.
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

## audio/ and nam-bench/ — the player's sound chain (M4)

Does the chain **timed events → SoundFont synthesiser → NAM → audio device** fit together, in real time, without allocating on the audio thread, and with sample-accurate timing? Measurements and conclusions: [docs/design/player.md](../docs/design/player.md), "The spike".

`audio/` (its own workspace): a four-bar test song (a guitar on six channels, one per string, with a bend; a bass; drums) rendered by rustysynth and OxiSynth, one synthesiser per track, in 64-sample callbacks split at every event, with a counting allocator around the audio work.

| Command | What it checks |
| --- | --- |
| `render rusty/N\|oxi SF OUT.wav` | Speed, worst callback, allocations; writes the mix and the guitar alone. `N` is rustysynth's block size. SF3 files are decoded to SF2 in memory first (`src/sf3.rs`). |
| `midi OUT.mid`, then `fluidsynth -n -i -R 0 -C 0 -r 48000 -F REF.wav SF OUT.mid` | The same song for FluidSynth, the reference. |
| `compare A.wav B.wav` | How close two renders are: loudness envelope correlation and spectral shape distance. |
| `timing rusty/N\|oxi SF` | How late a note starts against the sample it was asked for. |
| `nam MODEL IN.wav OUT.wav` | nam-rs on a guitar stem: speed per block size, worst block, allocations. |
| `pipeline SF MODEL OUT.wav` | The whole chain in 64-sample callbacks: three tracks, the guitar through NAM, mixed and interleaved. |
| `dls FILE` | The outline of a DLS sound set (Windows' `gm.dls`, macOS's `gs_instruments.dls`). |
| `devices`, `stream SF` (feature `devices`) | cpal's hosts and devices; three seconds played through the default device, rendering in its callback. |

`nam-bench/`: both NAM engines on the same plucked-string input, 64-sample blocks. NeuralAmpModeler-rs (feature `nam2`) needs x86_64, Rust 1.99 and `RUSTFLAGS="-C target-cpu=x86-64-v3"`.

The spikes workflow runs both on every target and prints the measurements as notices.
