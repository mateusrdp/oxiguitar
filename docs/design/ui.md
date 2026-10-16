# Interface design rules

These rules come from Mateus's review of the GUI spike (16 October 2026). They apply to `oxiguitar-app` from M1 on, and to any plugin that adds panels or palette entries. The Slint spike in `spikes/gui/slint` follows them and is the reference until the app exists.

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

  These follow Mateus's description (16 October 2026); before building them in M1 we check each against the Guitar Pro 8 user guide, so the four behave as Guitar Pro users expect.
- **Dark Page** (VIEW-8): light notation on a dark page. Page colours are themes of the display list (`Theme::LIGHT`, `Theme::DARK` in `oxiguitar-display`), so the PDF can use them too.
- **Multitrack** (VIEW-4) and the panels (Palette, Tracks).

## Tools menu

The Tools menu lists the tools of `oxiguitar-tools` in order, each with its description as a tooltip ([tools.md](tools.md)). An entry that cannot run on the open score is greyed out, with the reason in its tooltip. Picking one applies it and shows its report in the status line. Plugins add their tools to the same menu from M5.

The first tool is **Infer Key Signatures** (16 Oct 2026).

## Palette

- Tools are grouped (Note values, Effects, and many more later). Each group has a small heading with a chevron; clicking it collapses or expands the group, and the state is kept while searching.
- A compact search field at the top of the palette filters tools by name (Ctrl + K focuses it).
- Buttons are 32 × 32 logical pixels, wrapping to the panel's width.
- **Icons are drawn by our renderer, not typed as text.** Each symbol is measured by its ink, scaled, and centred so the centre of its ink's bounding box is the centre of the button, with a margin all round. Note values share one scale so their sizes compare; other symbols are fitted one by one; text labels (H, P.M.) share one size. Icons are white on transparent and tinted by the toolkit, and redrawn for the screen's pixel density and the interface zoom.
- Every button has a tooltip with its name and shortcut. Selection is shown by the button's highlight only.

## Accessibility

Keyboard use is required: every tool has a shortcut (UI-2) and Tab moves through controls. Screen-reader support is a late nice-to-have, not a goal for v1.0 (decided 16 October 2026); Slint's AccessKit support keeps the door open.
