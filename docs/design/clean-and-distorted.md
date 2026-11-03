# Clean and distorted sounds, per track (#152)

Status: decided by Mateus, 3 Nov 2026 (his answers follow each question, **Q**); not built yet.

## Where things stand

- The MIDI plugin (`oxiguitar.midi`) has one sound-set pick per role in its settings window (`settings.json`, `oxiguitar.midi`): `electric-guitars`, `acoustic-guitars`, `basses`, `drums`, `other`. A track's role comes from its General MIDI program (`settings::Role::of`): 24–25 acoustic, 26–31 electric, 32–39 bass. So a clean and a distorted electric guitar in one song get the same set, and a song of distorted guitars sounds wrong with a clean set picked.
- The MIDI plugin has no track section: nothing per track.
- The NAM plugin (`oxiguitar.nam`) already works per track: its track section "Amp (NAM)" keeps on/off, model, cabinet (automatic, none, built-in or a `.wav` in the `nam` folder), input and output levels in the song's sidecar `FILE.oxg`. It has no equalizer.
- The interface already carries what per-track choices need: an instrument and an effect get each track's settings for their own plugin (`PreparedTrack::settings`, the track section's JSON), and a change in the track dialog makes a new playing plan, so the track is prepared again (`plugin_source::Plan`). **Nothing below needs interface 1.5**, except where Q4 says so.
- The sound-set installer (oxiguitar-bettersounds-soundfonts) installs `clean-guitar`, `overdriven-guitar` and `distorted-guitar`; each set's `oxiguitar-set.json` says role `electric-guitars` today.

## The roles

General MIDI names the electric guitar's kinds by program: 26 jazz, 27 clean, 28 muted, 29 overdriven, 30 distortion, 31 harmonics. Guitar Pro files keep the program, so most songs already say which guitar is clean and which distorted.

Proposal: the settings window's "Electric guitars" row becomes three rows, one per set the installer offers:

| Row | Key | Programs |
| --- | --- | --- |
| Clean guitars | `clean-guitars` | 26, 27, 28, 31 |
| Overdriven guitars | `overdriven-guitars` | 29 |
| Distorted guitars | `distorted-guitars` | 30 |

The old key `electric-guitars` is still read: it fills any of the three rows that has no pick of its own, as 0.9's `guitars` fills both guitar rows today. A row with no pick plays the best General MIDI set, whose own programs 29 and 30 already overdrive and distort. `oxiguitar-set.json` uses the new keys (the installer's three sets say `clean-guitars`, `overdriven-guitars`, `distorted-guitars`); `electric-guitars` in an older file means "any of the three".

**Q1.** Three rows (clean, overdriven, distorted, matching the installer and General MIDI) or two? **Three rows.**

**Q2.** Basses: General MIDI has no distorted bass (32 acoustic, 33 finger, 34 pick, 35 fretless, 36–37 slap, 38–39 synth), so a global row cannot know a distorted bass from a clean one. Either one Basses row stays and a distorted bass is a per-track choice (its own set, or a clean bass through NAM), or a second row "Distorted basses" exists only for tracks that pick it. **One Basses row; a distorted bass is a per-track choice.**

## Per track: the sound

The MIDI plugin gains a track section "Sound" (interface 1.3 track sections, saved in `FILE.oxg` under `oxiguitar.midi`), one choice:

- **Automatic** (the default): the row of the settings window that the track's program names, shown with the set it gives ("Automatic: Distorted guitars, Black and Green").
- **As a role**: "Like clean guitars", "Like overdriven guitars", "Like distorted guitars", "Like basses", and so on: the track plays that row's set whatever its program says. Saved as `{"sound": "role:clean-guitars"}`, so it means the same on another computer.
- **A set**: any set found, by name. Saved by its path, `{"sound": "/path/to/set.sfz"}`. If the song is opened where that set isn't, the track plays as Automatic and the footer says so once.

**Q3.** Offer both "as a role" and "a set" per track, or only one of them? Roles travel with the song; sets are exact but only on the computer that has them. **Both.**

## Per track: the amp, and mixing clean and distorted

Mixing is already possible per track: each track has its own sound (above) and its own amp (the NAM section). The common case the issue names, a clean signal into a NAM model that distorts it, needs the track's sound to be clean while its program may say "distortion":

- Without an interface change: the user sets the track's Sound to "Like clean guitars" (or a DI set such as FreePats' Direct); the NAM section says so under its model picker while the amp is on and the Sound is Automatic on a distorted program. The NAM section can't see the MIDI section's settings, so its hint can only say what to do, not check it; the MIDI plugin can't see that the amp is on.
- With interface 1.5: the instrument would be told which effects are on for each track, and Automatic would pick the clean row by itself for a track with its amp on.

**Q4.** Manual (no interface change; the hint says what to do) or automatic (wait for interface 1.5)? **Manual, with the hint.**

## The plain case

A track with nothing picked plays as today: Automatic sound, amp off. That stays the default everywhere, so a computer that can't run NAM, or a user who never opens the track dialog, gets the role's set (now clean, overdriven or distorted as the program says) and nothing else.

## The equalizer: a plugin of its own

Mateus (3 Nov 2026): the equalizer belongs in the NAM plugin only if it is tied to the amp (NAM's crates, or controls that are about the amp). Ours is neither: plain biquad filters (Robert Bristow-Johnson's cookbook formulas), our own code, no library. So it is **a plugin of its own**, `oxiguitar.eq` in a new repository `oxiguitar-eq`, an effect with a track section "Equalizer", and it **works with the amp off too**: a SoundFont or SFZ track can be shaped by it alone, where NAM can't run.

Per track, saved in `FILE.oxg` under `oxiguitar.eq`:

```json
{"mode": "basic", "basic": [0, 0, 0], "graphic": [0, 0, 0, 0, 0, 0, 0, 0, 0, 0]}
```

- **Mode**: Off (the default), Basic or Graphic. Both modes keep their values; only the one shown plays, so switching back and forth loses nothing.
- **Basic**, three bands like an amp's tone stack: Bass (low shelf, 120 Hz), Middle (peak, 700 Hz, Q 0.8), Treble (high shelf, 3.2 kHz).
- **Graphic**, ten octave bands at 31, 62, 125, 250, 500 Hz, 1, 2, 4, 8 and 16 kHz (peaks, Q 1.41, one octave wide).
- Every band −12 to +12 dB in half-decibel steps (kept as proposed). A **Flat** button sets the shown mode's bands to 0.
- Sliders, as the NAM section's Input and Output are; ten in rows is long but plain (`docs/design/ui.md`: keep it minimal). A drawn curve would need a canvas, which track sections don't have.
- Zero latency; at most ten biquads a sample per track. Offered for every class of instrument.

## The order of a track's effects

A track's effects run in the order the host finds their plugins, which nothing fixes today; with the amp and the equalizer in two plugins, the order is heard. Mateus (3 Nov 2026): **the user orders them in the track dialog**, a core feature with no interface change: each effect's section can be moved up or down, the order is saved in `FILE.oxg` (the track's list of effect plugin ids), and the plan runs the effects in that order. Without a saved order, the core's default puts the effects it knows in a fixed order (`oxiguitar.nam`, then `oxiguitar.eq`) and any other after them in the order found.

## The pull requests, in order

Small and stacked, roles and per-track choice first:

1. oxiguitar-midi: the three electric guitar rows, the old key read for all of them, `oxiguitar-set.json`'s new keys; tests.
2. oxiguitar-bettersounds-soundfonts: the three electric sets' role keys (the repository must be attached to that session).
3. oxiguitar-midi: the track section "Sound" (Automatic, roles, sets); tests with the contract suite's test set.
4. oxiguitar: the order of a track's effects in the track dialog, saved in the sidecar, and the default order.
5. Mateus creates `oxiguitar-eq` with `scripts/new-repo.sh`; then oxiguitar-eq: the filters and settings with unit tests against the cookbook's responses, then the section, release and publish workflows.
6. oxiguitar-nam: the hint under the model picker (Q4).
7. oxiguitar: the manual (better-sounds.md, amp-models.md, playback.md, official-plugins.md) and this note marked as built.
