# Tools

The Tools menu holds actions the user starts that change the score. They live in `oxiguitar-tools`.

## Rules

- **Nothing runs on its own.** Opening, showing and exporting a score use what the file says. A tool changes the score only when the user picks it from the menu (Mateus, 16 Oct 2026: "If the user didn't make it explicit, don't try to be too smart. Let the user decide.").
- **One shape for every tool**, the `Tool` trait: an id (`infer-key-signatures`), a menu name, a one-sentence description for the tooltip and the manual, `availability` (the menu greys the entry out and shows the reason when the tool cannot run), and `apply`, a pure function from a score to a changed copy and a short report for the footer.
- **Built for plugins.** Tools depend on the score model only and never on the window, so the plugin system (M5) can host tools of the same shape, and any built-in tool can move into a plugin unchanged.
- **Undoable.** In the viewer (M1), a tool changes the open score; saving it needs the writers of M2. In the editor (M2), applying a tool is one command, undone in one step.

The command line runs the same tools: `oxiguitar-cli tools` lists them and `oxiguitar-cli render FILE --tool ID` applies one before engraving.

## Infer Key Signatures

Most tabs leave every bar in C major, Guitar Pro's default, whatever the music, and some set keys that do not fit it. This tool sets key signatures from the notes, with key changes where the music modulates. It is always offered (Mateus, 16 Oct 2026), even when the score sets keys, in case they are wrong: the inferred keys then replace the score's, and the report says nothing changed when they already agree. Only a score without bars greys it out.

1. For each bar, the music in each pitch class is counted across every pitched track, at written pitch, weighted by duration (a note counts for at most a half note; grace notes a sixteenth; tied and dead notes and drums not at all).
2. For each key from six flats to five sharps, a bar costs the share of its music outside that key's scale. G flat major stands for F sharp major, as songs in E flat tuning are usually written.
3. A best path through the song (Viterbi) picks one key per bar. Changing key costs as much as two and a half bars wholly out of key, one and a half where a section starts (a marker, a repeat, after a double bar), plus a fifth of a bar per step round the circle of fifths. A chromatic run therefore stays in the key, while a modulation that lasts changes it where it happens. Ties go to fewer accidentals.
4. The keys are used only if they spare at least 2.5 % of the music its accidentals compared with C; otherwise the music is written in C (chromatic riffs), and the report says so.

The report names each key with its relative minor, such as "Key signatures set: G major or E minor (1♯) in bars 1–102, E♭ major or C minor (3♭) in bars 103–153."

On the private corpus, the tool agrees with every file that sets its own single key (Wish You Were Here, Little Wing, Shine On You Crazy Diamond, The Loner, Reelin' in the Years) and finds the modulations of The Bigger Picture and Sardinha Desvairada bar for bar. Where it differs, it picks a relative or neighbouring key (Surfing with the Alien's modal passages, for one). `cargo test --release -p oxiguitar-tools --test corpus -- --nocapture` prints its report for every corpus file.
