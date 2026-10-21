# Tools and changes to the score

Since interface 1.2, a plugin can add entries to the Tools menu and change the open score. Every change goes through the core, as one step the user can undo and redo: a plugin never changes the score behind the editor's back.

## Tools

List each tool under `tools` in the manifest, with an `id`, the menu entry's `name` in title case, and a one-sentence `description` (`oxiguitar-cli tools` shows it):

```json
"tools": [
  { "id": "octave-up", "name": "Up an Octave", "description": "Moves every note of the score up an octave." }
]
```

Your `Plugin` hands out each one from `tool(id)`, as something implementing `Tool`. When the user picks the entry, the core calls its `run`, on a thread of the core's: return a sentence for the footer about what it did ("Moved 312 notes up an octave."), or an empty one to say nothing, or an error saying why it couldn't run. A tool runs with no score open too (the contract suite tries it), so check.

## Changing the score

A tool, a tab's button or anything else in your plugin changes the score the same way:

1. Ask for it: `host.score2()` gives the score, version 2, and its **revision**, a number that changes with every change to the score (score events carry it too). `None` when no score is open.
2. Change your copy.
3. Hand it back: `host.apply(revision, "Up an Octave", score)`. The name is what the Edit menu shows ("Undo Up an Octave"), a few words.

The core takes the whole score and records the difference as one change; the page shows it a moment later, and plugins that listen hear `Event::ScoreChanged`. If the score changed since your revision (the user typed a note while your tool worked), the core refuses the change with an error: ask for the score again and redo it, or tell the user. A score the core can't hold (a staff missing bars, a fraction with a zero denominator) is refused the same way.

## The selection

Built for 1.2, a plugin that lists `selection` under `events` hears `Event::Selection2`, the selection as the editor has it, instead of 1.1's `Event::Selection`:

- `Caret { track, staff, voice, bar, beat, string }`: nothing selected, the caret on a beat of a voice (beats count in their own voice) and on a string of the tablature (from 1, the highest; 0 on a staff without strings);
- `Beats { track, staff, voice, from, to }`: a run of beats of one voice;
- `Bars { first_track, last_track, first_bar, last_bar }`: whole bars across tracks;

and `None` when no score is open. Everything counts from 0 but the string.

## The score, version 2

`host.score2()`, `Host::apply`, and the readers and writers of `reader2` and `writer2` ([Formats](formats.md)) use `oxiguitar_plugin::score2`: version 1's tree, each part ending in a list `ext` of extensions. Read the ones you know (`MasterBar2::free_time()`), keep the others as they are when you hand a score back, and add only what your format holds.
