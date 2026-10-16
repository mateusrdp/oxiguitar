// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Generated songs read back through the real Guitar Pro reader, with
//! nothing lost and nothing to report.

use oxiguitar_model::{BeatKind, NotePlace};
use oxiguitar_testgen::{band_song, to_gp};

#[test]
fn the_band_song_reads_back_whole() {
    let song = band_song(48);
    let imported = oxiguitar_formats::read(&to_gp(&song)).expect("the reader opens it");
    assert!(
        imported.log.is_empty(),
        "the reader had something to say: {:?}",
        imported.log.entries()
    );
    let score = &imported.score;
    assert_eq!(score.master_bars.len(), 48);
    assert_eq!(score.tracks.len(), 8);
    for (track, made) in score.tracks.iter().zip(&song.tracks) {
        assert_eq!(track.name, made.name);
        let staff = &track.staves[0];
        assert_eq!(staff.bars.len(), 48, "{}", track.name);
        // Every bar of every track has notes, as written.
        for (bi, bar) in staff.bars.iter().enumerate() {
            let notes = bar
                .voices
                .iter()
                .flat_map(|v| &v.beats)
                .filter(|b| b.kind != BeatKind::Rest)
                .count();
            assert!(notes > 0, "{} bar {}", track.name, bi + 1);
        }
    }
    // The lead stays on strings and frets.
    let first = &score.tracks[0].staves[0].bars[0].voices[0].beats[0].notes[0];
    assert!(matches!(first.place, NotePlace::Fretted { .. }));
    assert_eq!(score.tracks[3].lyrics.len(), 0);
    assert_eq!(score.tracks[5].lyrics.len(), 1, "the voice has lyrics");
}
