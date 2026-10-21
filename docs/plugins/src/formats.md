# Formats: reading and writing files

A format plugin turns a file into a score, a score into a file, or both. OxiGuitar has no format of its own: what it opens is what the installed plugins read.

## Declaring a format

```json
"formats": [
  {
    "id": "extab",
    "name": "Example tab",
    "extensions": ["extab"],
    "media_types": ["text/x-example-tab"],
    "read": true,
    "write": true,
    "signatures": [{ "at": 0, "text": "EXAMPLE TAB" }]
  }
]
```

| Key | What it is |
| --- | --- |
| `id` | The format's id in the plugin; requests name it |
| `name` | Shown in dialogs and messages |
| `extensions` | Lowercase, without the dot, the usual one first: for the Open and Save dialogs |
| `media_types` | Optional, such as `application/x-gtp` |
| `read`, `write` | What the plugin does with it; at least one |
| `signatures` | How to recognise its files; a format that reads needs at least one |

### Signatures

OxiGuitar recognises a file by its **contents**, from the manifests, before any plugin is loaded. Extensions are never used for that: they lie (four of the Guitar Pro 3 files in our test corpus are Guitar Pro 5). A file is of your format when any of its signatures matches, and a signature matches when every test it gives holds:

| Test | Matches when |
| --- | --- |
| `"text": "FICHIER GUITAR PRO v5"` | This UTF-8 text is found at byte `at` (default 0) |
| `"hex": "42434658"` | These bytes are found at byte `at` |
| `"zip_member": "Content/score.gpif"` | The file is a ZIP archive holding a member of exactly this name |

Make a signature as narrow as the format allows: when two plugins' signatures match one file, the first plugin in folder order reads it. A plugin that declares an extension but doesn't recognise a file with it makes OxiGuitar ask the user for a bug report; a file nobody recognises gets a feature request instead. Both point at OxiGuitar's issue tracker, so tell users in your README where to report your plugin's bugs.

## Reading

```rust,ignore
pub trait FormatReader: Send + Sync {
    fn read(&self, data: &[u8]) -> Result<Read, Error>;
}

pub struct Read {
    pub score: score::Score,
    pub log: Vec<LogEntry>,
}
```

`data` is the whole file. **Treat it as untrusted**: any file a user has can be damaged or made on purpose to break readers. Every problem is an `Error` that says where (its `offset` in bytes, and a `context` such as "bar 12, track 2"), never a panic, an endless loop or an allocation of whatever size the file claims. The contract suite feeds every reader an empty file, a kilobyte of zeros and its own signature alone; fuzz yours too if it reads a binary format ([Testing](testing.md)).

The **log** is what the file holds that the score can't carry, or carries only roughly: an effect OxiGuitar doesn't have, a value out of range that was clamped, a bar that didn't add up. Report it rather than dropping it silently. Each entry has a `level` (`Info` when nothing was lost, `Warning` when something was), a `message`, the first `place` it happened, and a `count`: one entry per kind of problem, counted, not one per bar. `oxiguitar-cli info FILE` prints the log; the window doesn't show it yet.

The template's reader:

```rust,ignore
{{#include ../template/src/format.rs:read}}
```

## The score

The score crosses the interface as its own types, `oxiguitar_plugin::score` (version 1 of the wire score). They mirror OxiGuitar's model field for field, but belong to the interface: the model can change as the editor grows, and these don't.

A plugin built for interface 1.2 can read and write version 2 instead (`oxiguitar_plugin::score2`), which holds what OxiGuitar added since, such as bars in free time, as lists of extensions ([Interface versions](versions.md)). Implement `FormatReader2` and `FormatWriter2` and hand them out from `reader2` and `writer2`; with both versions, the SDK uses version 2. A plugin with only a version-1 writer still gets every score: what version 1 can't hold is added to its losses.

```text
Score
├── info: ScoreInfo          title, subtitle, artist, album, words, music, …
├── initial_tempo            quarter notes per minute
├── master_bars: [MasterBar] time and key signature, repeats, endings, markers,
│                            tempo changes, directions (segno, coda, D.S. …)
└── tracks: [Track]          name, instrument (General MIDI), mix, chord diagrams, lyrics
    └── staves: [Staff]      tuning, capo, frets, notation and tab shown (two staves: a grand staff)
        └── bars: [Bar]      the clef; one bar for each master bar
            └── voices: [Voice]  one to four
                └── beats: [Beat]  a duration; notes, a rest or nothing; beat effects; a chord diagram
                    └── notes: [Note]  string and fret (or a pitch); tie, dynamic, note effects
```

The conventions:

- **Strings are numbered from 1, the highest.** `Staff::tuning` lists the open strings' pitches from string 1 down; standard guitar tuning is `[64, 59, 55, 50, 45, 40]`.
- **Pitches are MIDI note numbers**, 60 = middle C.
- **Time is exact.** Positions and lengths are `Ratio`s of a whole note (`num / den`, `den` > 0); durations are a note value, dots and tuplets, outermost first.
- **The shape must hold**: every track has at least one staff; every staff has exactly one bar per master bar; every bar has at least one voice. A voice's beats should fill its bar's time signature; `oxiguitar-cli info` checks that ("Bar check").
- **Show what the file says.** Don't infer keys, add notation the file didn't ask for or fix the music; OxiGuitar has tools for that, which the user chooses.

Several types have no default (a master bar's time signature, a beat's duration, a note's place, a bar's clef), so you fill them in. The template's score:

```rust,ignore
{{#include ../template/src/format.rs:score}}
```

`oxiguitar-cli info FILE` and `oxiguitar-cli render FILE --png page` show what OxiGuitar makes of your score, faster than the window ([Testing](testing.md)).

## Writing

```rust,ignore
pub trait FormatWriter: Send + Sync {
    fn write(&self, score: &score::Score) -> Result<Written, Error>;
}

pub struct Written {
    pub data: Vec<u8>,
    pub losses: Vec<LogEntry>,
}
```

A writer returns the whole file, and in `losses` everything the format can't hold and left out, like the reader's log: the user is told before anything is lost. It must answer any score the core can hold, the empty one included (the contract suite writes one). The template's:

```rust,ignore
{{#include ../template/src/format.rs:write}}
```

Writers are declared and called through the plugin host today, and the contract suite checks them; the window's Save and Export menus use them from the editor's release (OxiGuitar 0.5).

## One format per plugin

Our own plugins each read one format (Guitar Pro 3, 4, 5, 6 and 7–8 are five plugins), so a format can be added, improved or retired on its own, and a user loads only the code for files they open. You may put several formats in one plugin; consider whether your users would rather have them apart.
