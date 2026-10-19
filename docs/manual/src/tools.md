# Tools

The **Tools** menu holds actions that change the open score. They only run when you choose them: OxiGuitar never changes a score on its own. A tool changes the score on screen, never the file on disk, until you save it; each tool's change is one step that **Edit → Undo** takes back.

A tool that cannot run on the open score is greyed out, with the reason in brackets after its name. After OxiGuitar's own tools come those your plugins add, if any ([Settings and plugins](plugins.md)); `oxiguitar-cli tools` lists them all.

## Infer Key Signatures

Most tabs leave every bar in C major, so their notation is full of sharps and flats. **Tools → Infer Key Signatures** works out key signatures from the notes, with key changes where the music modulates, and replaces any the score already has. Music that stays chromatic is left in C.

Try it when a song's notation looks crowded with accidentals; if you don't like the result, undo it.
