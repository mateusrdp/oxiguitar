# Harmonize: chords found for a melody (#235)

A track has a melody; the user wants chords over it. #235 asks for a tool that reads the music and writes the chords, and names three systems to look at. This is what they are, why none of them is ported, and what was built instead.

## Decided by Mateus (10 Nov 2026)

- **The method:** our own search for the cheapest chain of chords (Viterbi), with numbers counted in a dataset we may use, rather than a port of one of the three references.
- **What it writes:** chord names with their diagrams above the caret's track, as if entered with the chord builder. They sound when the track's "play chords" is on (#151), and each can be edited.
- **How it starts:** a small dialog, as Fit Bars has. Three choices: plain chords or sevenths too; one chord a bar or changes inside bars; **replace or keep the chords already there** (he asked for this one to be the user's choice).
- **What it hears:** the notes of every track, not only the caret's ("make sure it considers the notes on each beat across all tracks").

## Decided by Mateus (15 Nov 2026)

- **Every track or this track only:** a fourth choice in the dialog, "Listen to every track but drums" (the default) or "Listen to this track only".
- **The whole song:** a tickbox in the dialog, "Harmonize the whole song, not only the selected bars" (or "this bar"), so the page's right-click menu can harmonize every bar too. It starts ticked from Tools and unticked from the right-click menu. `Harmonize::whole_track` became `whole_song`: every bar of the song, with the chords on the caret's track as before.
- **Chord names as they sound** on a guitar tuned down or with a capo (a C shape tuned a semitone down is written B), as the chord builder names them.

Taken as plain defaults by the session: it lives in the core like Find Fingering and Fit Bars (the plugin interface has no tools yet); the key is the bars' key signature; one step for Undo.

## The three references

| Reference | How it works | What it needs | Why it isn't ported |
| --- | --- | --- | --- |
| [AutoHarmonizer](https://github.com/sander-wood/autoharmonizer) (Wu, Li, Sun; MIT code) | Two stacks of LSTM layers read the melody, the beat strengths and the key to the left and to the right of each sixteenth note and pick one of 1,510 chord names; a setting moves the "harmonic density" | 45 MB of trained weights | The weights were trained on Wikifonia's lead sheets, which are copyrighted songs. They are too big for the core, so it would be a plugin with a download of its own |
| [The PCFG harmonizer](https://pcfgharmonization.github.io/) (Tsushima, Nakamura, Yoshii, IEEE/ACM TASLP 2020) | A grammar for chord sequences, a Markov model for when chords change and one for the melody's notes over each chord; 5,000 samples drawn for each melody | Parameters trained on the Billboard, J-Total and Rock Corpus data | Its code has no licence. 4/4 and sections of eight bars only, 24 triads, a different answer every run. In its own paper the rule-based Melisma is right more often (22.1 % against 19.6 %) |
| [AccoMontage2](https://github.com/billyblu2000/AccoMontage2) (Yi, Hu, Zhao, Xia, ISMIR 2022; MIT code) | Whole progressions picked from a library of 5,000 and chained by dynamic programming, phrase by phrase | The tonic, the mode and the phrases ("A8B8A8B8") from the user | 4/4 only, phrases of four or eight bars only, and its library is re-organised from a commercial MIDI pack |

A comparison of such methods on one dataset ([Yeh et al., 2021](https://arxiv.org/abs/2001.02360)) puts them close together: template matching gives 29 % of half bars the song's own chord, a hidden Markov model 31 %, a bidirectional LSTM 35 %, the authors' best network 38 %. A melody has many good harmonizations, and the heavy models buy little. crates.io has no harmonizer to delegate to.

What is taken from the references is their ideas: the hidden Markov model both papers use as a baseline is the method; the PCFG paper's separate model for when chords change is the cost of holding or changing at a bar line and inside a bar; AutoHarmonizer's "harmonic density" is the dialog's second choice; and all three count chords from the key, as here.

## What exists

- **Chords** (`oxiguitar_edit::harmony`): Guitar Pro 8's builder (root, triad, extension, alterations, bass), a chord's name, its notes, and the ways to play it on any tuning (`Chord::voicings`), easiest first. A track keeps each diagram once and beats refer to it (`Beat::chord`).
- **Played chords** (#151): a track's chord diagrams sound, each until the next.
- **Keys:** each bar's key signature; Tools → Infer Key Signatures sets them from the notes of every track.
- **Two searches of the same shape:** Infer Key Signatures and Find Fingering are both a cheapest path through the song.

## The model

**Cells.** The bars are cut into cells: a bar each, or with "changes inside a bar" two halves where the bar has two halves of a dotted quarter or more (4/4, 2/2, 6/8, 12/8; not 2/4 or 3/4).

**Evidence.** The caret's track is the melody: how many quarter notes each pitch class sounds in the cell, and the notes struck where the cell starts (each for its length, a quarter note at most). With "every track" (the default), every other pitched track is the accompaniment: the share of the cell each pitch class takes of all those tracks play together (so three guitars count as much as one), and their lowest note. With "this track only" there is no accompaniment, and a cell where the caret's track is silent holds the chord. Drums, dead notes and grace notes don't count; tied notes count as sounding but not as struck. Pitches are as written (sounding pitch plus the staff's transposition), as Infer Key Signatures reads them.

**Chords.** A chord is a kind on a root: major, minor and diminished, and with "sevenths too" also maj7, m7, 7, m7(b5), sus4 and 7sus4. Chords are counted from the tonic of the major key the key signature shows. A song in A minor is harmonized with C major's numbers, in which the chord on the sixth degree is the commonest, so the tool never has to tell a major key from its relative minor.

**Costs** (minus logarithms of how often something came in the dataset):

- a quarter note of melody in each pitch class over each chord, and the same for the accompaniment and for its lowest note;
- each chord after each other chord, given a change;
- holding and changing the chord, at a bar line and inside a bar;
- each chord as the first of a song, and each chord anywhere (for a selection that starts later).

**The search.** The cheapest chain of chords through the cells, exactly (Viterbi): at most 108 chords and a few hundred cells, a few milliseconds. A chord may change only where the caret's staff has a beat starting with the cell (a chord sits on a beat) and something sounds; silent cells hold the chord. The chain runs from the first cell where something sounds to the last, so rests before and after the music get no chord.

**Chords kept.** With "keep", a cell that already has a chord is fixed to it (to the nearest chord of the vocabulary: C9 counts as C7, and as C with plain chords), and the chain passes through it. A name the builder can't read breaks the chain there. Kept chords are never rewritten; with "replace", every chord in the bars goes first.

**Writing.** A chord is written at the first cell and wherever it changes. Its root is spelled as the key spells it; roots between the key's degrees are flats (the flat third, sixth and seventh) or the raised fourth, and sharps under a diminished chord (a leading tone); a name the builder lacks (C flat) falls back to its plain one. The diagram is the easiest shape `Chord::voicings` gives on the staff's tuning and capo; a staff without strings gets the name alone.

**The key.** The bars' key signatures are trusted: the user sets them, and Infer Key Signatures finds them from every track. But most tabs leave C major whatever the music, so when one key signature covers the bars and another key makes the chain clearly cheaper (by 0.15 a cell; see the measurements) the footer warns: "The chords were found in C major or A minor, the key of the key signature, but the notes fit E major or C# minor better. If the key signature is wrong, run Tools → Infer Key Signatures and harmonize again." The tool never changes a key signature. (The footer keeps warnings and errors only, so a run that needs no warning says nothing: the chords are on the page.)

## The numbers

Counted in **POP909** (Wang, Chen, Jiang, Zhang, Xu, Dai, Bin, Xia, ISMIR 2020; [MIT licence](https://github.com/music-x-lab/POP909-Dataset)): 909 pop songs as MIDI with the melody and the piano accompaniment on separate tracks, chord labels, beats and keys. `scripts/harmony/measure.py` reads it and writes `crates/oxiguitar-edit/src/harmonize/numbers.rs` (104 KiB of tables, committed; nothing in a build or a release runs the script). 880 songs are usable (4/4, beats tracked well enough); every tenth is kept aside to tune four weights (melody, accent, accompaniment, lowest note) and to measure:

| | Melody alone | Melody alone, key heard by the model | With the accompaniment |
| --- | --- | --- | --- |
| Plain chords, one a bar | 32.3 % | 31.9 % | 86.7 % |
| Plain chords, changes inside bars | 33.0 % | 32.5 % | 90.5 % |
| Sevenths too, one a bar | 29.2 % | 28.8 % | 75.5 % |
| Sevenths too, changes inside bars | 29.5 % | 29.1 % | 87.2 % |

The figures are the share of cells that get exactly the song's chord. A third from the melody alone is what the literature's models reach (29 to 38 % above), and always playing the commonest chord gets 20 %. With the accompaniment the tool mostly reads the chords the band plays, which is what POP909's labels were made from, so those figures say the tables are sound, not that the tool is that good on other music.

Tried and dropped: a second-order model (the chord after the last two) was no better (32.2 to 32.7 % against 33.6 % on the same songs) and its table would be thirty times larger.

The key the model hears from the melody alone (the cheapest of the twelve) is the song's own in 94 to 95 % of songs. Where it was not, it was cheaper by 0.03 to 0.08 a cell; where it was, the next key was dearer by 0.2 to 0.6 (median). Hence the report's threshold of 0.15.

POP909's sevenths are sparse (most of its labels are triads), so "sevenths too" still writes mostly triads, with a seventh where the notes spell one.

## On the private corpus

`cargo test --release -p oxiguitar-edit --test harmonize_corpus -- --nocapture` (with the corpus in `corpus/files/` and the Guitar Pro plugins in `OXIGUITAR_PLUGINS_DIR`) harmonizes every pitched track of every file four ways and asserts what must hold: the score stays sound, only the caret's track changes and of it only its chords, every chord written is one the builder reads back, kept chords stay on their beats, Undo gives the file back. 104 files, 494 pitched tracks, 33,502 chords written with the plainest choices; the slowest run took 0.25 s (a debug build is slower).

Thirty-eight tracks have eight or more chords of their own. With plain chords, one a bar, **36 % of 3,939 bars get the file's chord and 47 % its root** (the file's roots moved by the tuning and the capo first, since tabs name shapes). That is far from POP909's 87 %, and the reasons are the limits below: power-chord riffs, more than one chord a bar in the file, and music that isn't pop.

**The key matters less than expected.** 78 of the 104 files draw the warning: tabs leave the key signature at C. Harmonizing those in the key the notes fit instead was tried: 38 % of bars and 48 % of roots, against 36 % and 47 %. With an accompaniment the notes carry the chord whatever key the numbers are read in; the key signature mostly decides how roots are spelled (A flat minor under no sharps, G sharp minor under four). So the tool keeps to the key signature, as decided, and warns.

In POP909 the cheapest key, heard with the accompaniment, was the song's own in every song kept aside, by 0.5 a cell or more; so the warning's 0.15 doesn't fire on a right key signature there.

## Where it lives

- `crates/oxiguitar-edit/src/harmonize.rs`: `Harmonize { sevenths, inside_bars, keep, all_tracks, whole_song }`, `Command::Harmonize`, `harmonize_report` (how many chords the command would write, and the warning about another key), the cells, the chain, the spelling and the diagrams; `harmonize/numbers.rs` (written by the script); `harmonize/tests.rs`.
- The app: `HarmonizeDialog` in `bar_dialog.slint` (four drop-downs and the whole-song tickbox); Tools → Harmonize… (the tickbox ticked) and the page's right-click menu (unticked: the selected bars, or the caret's bar); `open_harmonize` and `harmonize_dialog_done` in `main.rs`. The drop-downs' choices stay as they were for the next time, while the window is open; the tickbox follows the menu it was opened from.
- Undo: one change, of the track's chord list and the bars' beats.

## Limits, and possible later work (not asked for)

- **The style is pop.** The numbers come from 909 Chinese pop songs: bossa nova, jazz and metal riffs get pop's chords. The script can count any set of lead sheets in POP909's layout; a set of numbers per style would be a fourth choice in the dialog.
- **No slash chords, ninths or altered chords**; no chords with a bass other than the root.
- **One chord for at least half a bar.** Chords on every beat aren't found.
- **The melody is the caret's track.** A track that plays its own chords under its melody is read as melody throughout.
- **The key is never guessed**, only hinted at in the report.
- **Repeats and sections** aren't used: a verse that comes twice with the same notes gets the same chords only because the notes are the same.
- **Power chords.** Rock and metal tabs write A5; the tool writes A or Am, whichever the other notes suggest, and chromatic riffs get diminished chords nobody would write. POP909 has no power chords to count.
- **Chords are named as they sound.** On a guitar tuned a semitone down, a C shape is written B, as the chord builder names it; many tabs name the shape instead. The same goes for a capo.
