# Reference songs as Guitar Pro 7.6 saves them

Ten of the reference songs (`fixtures/reference/`, our own, made by `oxiguitar-testgen`), opened and saved in Guitar Pro 7.6 by Mateus on 19 Oct 2026 (Guitar Pro 7.6.0 under Wine 11 on Debian sid). Only the score XML of each file (`Content/score.gpif`) is kept: the rest of a Guitar Pro file is its stylesheet and view settings, which are Guitar Pro's, not the song.

They show how Guitar Pro writes what we write, for the .gp writer (M3 slice 2), and `oxiguitar-testgen`'s `guitar_pro_7_6` test reads each one back against our original.

What Guitar Pro 7.6 did with our files:

- It kept every note, beat, effect, bar, tempo and header text.
- It dropped the chord diagrams: it keeps a diagram only with its chord spelled out (key note, bass note, intervals), and ours had a name and frets only.
- It dropped the text with a tempo ("Moderato").
- It wrote its own mixer settings and a sound from its library for each track (a path such as `Stringed/Electric Guitars/Clean Guitar`, with its sound engine's settings), where we wrote only a General MIDI program.
- Two files it could not open at all: `lyrics.gp` and `eight-tracks.gp` crash Guitar Pro 7.6's sound engine (`gprse`). Both have a voice track (General MIDI program 54) with no sound from Guitar Pro's library, which the writer must always give.

## Files made in Guitar Pro 7.6

Two whole files Mateus made in Guitar Pro 7.6 on 20 Oct 2026, to show how Guitar Pro itself writes what our first files got wrong (`crates/oxiguitar-formats/tests/guitar_pro_7_6_own.rs`):

- `nylon-guitar-default.gp`: a nylon guitar with both E strings played open. Guitar Pro's octave is MIDI ÷ 12 (MIDI 64 is E5, written E6), one above scientific pitch; we wrote E4 and E5, and Guitar Pro drew the staff an octave low.
- `pianos-high-low.gp`: six piano tracks (two named violin and voice) with notes at MIDI 0, 53, 67 and 127. Every staff has a tuning (a guitar's on the first staff, a five-string bass's on the second), and every note a string and fret, out of range where it must be (fret −40 for MIDI 0, 63 for MIDI 127); we wrote a tuning of zeros and no strings, which Guitar Pro drew as CCCCCC with frets of −2147483648.
