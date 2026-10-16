# Interface design rules

These rules come from Mateus's review of the GUI spike (16 October 2026). They apply to `oxiguitar-app` from M1 on, and to any plugin that adds panels or palette entries. The app (`crates/oxiguitar-app`, binary `oxiguitar`) follows them; the Slint spike in `spikes/gui/slint` stays as the reference for the palette, which arrives with the editor in M2.

## The M1 window (the viewer)

- **Menus:** File (Open… Ctrl + O, Export PDF…, Paper Size: A4 or Letter, Quit Ctrl + Q), View, Tools, Help (Keyboard and Mouse F1, About). There is no Edit menu yet: in a viewer all its entries would be greyed out, so it comes with the editor in M2, as does the palette (F7).
- **Canvas:** the score, centred on a grey (dark grey with Dark Page) background, with scroll bars. The mouse wheel scrolls; in the Horizontal layout, when the line fits the window's height, the wheel scrolls along it. Arrow keys, Page Up / Page Down, Space, Home and End scroll too. Before a file is open, one muted line says how to open one.
- **Score zoom slider** bottom right, as below. Zooming keeps the middle of the view in place. In the Vertical layout the lines are as wide as the window, so zooming or resizing reflows the bars.
- **Status line** bottom left, a pill like the slider: errors (a file that cannot be read, a MuseScore file that needs its plugin) and tool reports. It disappears after ten seconds or when clicked. Nothing is said when things simply work.
- **Track list** along the bottom (F8; hidden from View → Tracks): number, a tick box to show the track, the name, the instrument (strings or drums), and two **notation buttons**: a G clef for standard notation and the vertical TAB sign for tablature (tracks without strings have only the clef). The buttons are drawn by our renderer, centred on their ink and highlighted when on, like the spike's palette buttons (`icons.rs`). Any combination is allowed: standard notation, tablature, both, or neither, which hides the track and unticks it; ticking a hidden track brings back its last notation, and a notation button on a hidden track shows it (Mateus, 16 Oct 2026). The last shown track cannot be hidden. Tablature starts on only for fretted instruments (see engraving.md, "Clefs, tablature and keyboards"). Clicking a name shows that track alone. View → Multitrack is ticked whenever more than one track is shown; clicking it goes back to the first of them, or from one track shows all. Its height fits up to five tracks and the splitter changes it. The rules live in `tracks.rs`, without window code, and are unit-tested.
- **Ticks in menus** are two-way bound in Slint and set from one place (`pick-layout`, `pick-paper`): a checkable item flips its own tick when clicked, which breaks a one-way binding such as `checked: layout == 0`, so the layout and paper items behave like radio buttons only this way.
- **Window title** is just "OxiGuitar" (no file name), as Mateus asked.
- **Icon:** the placeholder pick (`assets/icon/`, PNGs in `assets/icon/png/`) is the window icon on X11, Windows and macOS. Wayland compositors ignore window icons: they show the icon of the desktop entry named by the window's app id, `io.github.mateusrdp.OxiGuitar` (`assets/linux/`). The packages will install the entry and icons; for a build from source, `scripts/install-desktop-entry.sh` installs them for the current user.
- **Open and Export dialogs** are the system's own, through the XDG desktop portal on Linux; they run on their own thread so the window keeps drawing (on macOS on the main thread, as the system requires). If the session has no D-Bus for the portal, the status line says so and suggests `oxiguitar FILE`. A file named on the command line opens at start, which is also how file managers open files (UI-4). A file dropped on the window opens too (winit's `DroppedFile`: Windows, macOS and X11; winit 0.30 and Slint 1.18 have no file drops on Wayland, so a Wayland drop needs our own data-device handling or a newer winit); of several files, the first opens.
- **PDF export** writes the tracks shown, in the Page layout, on the paper chosen under File → Paper Size, always on a light page (EXP-1), next to the score by default.
- **Drawing:** a worker thread reads, engraves and draws 512-pixel tiles at the screen's pixel density; the window shows them as they arrive, nearest the middle of the view first, and keeps those near the view. Scrolling never waits for engraving.

## Principles

1. **Minimal.** Screen space belongs to the score. Every visible control must earn its place; when in doubt, leave it out, put it in a menu, or make it a shortcut.
2. **No labels that state the obvious.** No app name inside the window (the title bar says "OxiGuitar"), no caption repeating what a highlight already shows, no "Tracks" heading over the track list.
3. **The top of the window is for menus only:** File, Edit, View, Help (and later Track, Note, Tools, Playback, as Guitar Pro has). No toolbar rows of buttons there. Display options live under View or in Preferences.
4. **The user decides what is shown.** Panels can be hidden from the View menu and with a key (F7 palette, F8 tracks in the spike). Palette tools come in groups that collapse and expand with a click on their heading.
5. **Look like Guitar Pro 8, without copying it** (UI-1, UI-3): same places for things, same shortcuts, our own drawing.

## Zoom

There are two zooms, and they never share a control.

| Zoom | What it scales | How | Visible control |
| --- | --- | --- | --- |
| Interface | Everything in the window: menus, palette, panels, text | Ctrl + mouse wheel; Ctrl + `+` (or `=`) in, Ctrl + `-` (or `_`) out, Ctrl + `0` back to 100 %; numeric keypad too | None. Documented in Help → Keyboard and Mouse and in the manual. |
| Score | The score canvas only, 25 % to 400 % | A slider in the bottom-right corner of the canvas | The slider: logarithmic, so 100 % sits in the middle; it snaps to 100 % near the middle; clicking its percentage returns to 100 %. About 170 px wide in all. |

Interface zoom follows browser steps (50, 67, 75, 80, 90, 100, 110, 125, 150, 175, 200 %) and multiplies the system's scale factor, so it stays right when the window moves to another monitor. It is caught before any widget sees it, so Ctrl + wheel over the score never scrolls it.

## View options

Everything about how the score is shown lives in the View menu:

- **Layout** (VIEW-3, M1): how staves are laid out, as in Guitar Pro.
  - **Page**: systems fill the page width, stack vertically and break into pages.
  - **Vertical**: systems fill the window width and stack vertically, with no pages.
  - **Horizontal**: one single long line, scrolling sideways.
  - **Parchment**: one single long vertical column, with no page breaks.

  These follow Mateus's description (16 October 2026), checked against the Guitar Pro 8 user guide before they were built: they are GP8's Page – Vertical, Screen – Vertical, Screen – Horizontal and Page – Scroll ([engraving.md](engraving.md#layouts-view-3)).
- **Dark Page** (VIEW-8): light notation on a dark page. Page colours are themes of the display list (`Theme::LIGHT`, `Theme::DARK` in `oxiguitar-display`), so the PDF can use them too.
- **Multitrack** (VIEW-4) and the panels (Tracks now; Palette with M2).
- **Dynamics**: shows or hides dynamics markings (on by default), for scores where they clutter the page.

## Tools menu

The Tools menu lists the tools of `oxiguitar-tools` in order ([tools.md](tools.md)). An entry that cannot run on the open score is greyed out. Slint's menus have no tooltips, so the reason follows the name in brackets, such as "Infer Key Signatures (the score already has key signatures)"; descriptions go in the manual. Picking one applies it and shows its report in the status line. The file on disk is never changed: reopening it shows what it says. In M2 applying a tool becomes one undoable command. Plugins add their tools to the same menu from M5.

The first tool is **Infer Key Signatures** (16 Oct 2026).

## Palette

- **Tuplets** (EDIT-10, M2): a palette tool with a shortcut makes triplets; "Other…" opens a small dialog to choose any grouping, n notes in the time of m, with the usual m filled in (Mateus, 16 Oct 2026). Shortcuts follow Guitar Pro's, checked against the GP8 user guide when the palette is built.
- Tools are grouped (Note values, Effects, and many more later). Each group has a small heading with a chevron; clicking it collapses or expands the group, and the state is kept while searching.
- A compact search field at the top of the palette filters tools by name (Ctrl + K focuses it).
- Buttons are 32 × 32 logical pixels, wrapping to the panel's width.
- **Icons are drawn by our renderer, not typed as text.** Each symbol is measured by its ink, scaled, and centred so the centre of its ink's bounding box is the centre of the button, with a margin all round. Note values share one scale so their sizes compare; other symbols are fitted one by one; text labels (H, P.M.) share one size. Icons are white on transparent and tinted by the toolkit, and redrawn for the screen's pixel density and the interface zoom.
- Every button has a tooltip with its name and shortcut. Selection is shown by the button's highlight only.

## Accessibility

Keyboard use is required: every tool has a shortcut (UI-2) and Tab moves through controls. Screen-reader support is a late nice-to-have, not a goal for v1.0 (decided 16 October 2026); Slint's AccessKit support keeps the door open.
