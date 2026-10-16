// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Engraving tests on the public fixtures, on damaged scores and, where
//! `corpus/files/` exists, on the private corpus (skipped quietly otherwise).
//!
//! With the corpus in place:
//! `cargo test --release -p oxiguitar-layout --test engrave -- --nocapture`

use std::path::{Path, PathBuf};

use oxiguitar_display::smufl::Metadata;
use oxiguitar_display::{DisplayList, FontSet, Item};
use oxiguitar_layout::{Engraved, Options, engrave};
use oxiguitar_model::{
    Bar, Beat, BeatKind, Duration, MasterBar, Note, NotePlace, NoteValue, Pitch, Score, Staff,
    TimeSignature, Track, Tuplet, Voice,
};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn setup() -> (FontSet, Metadata) {
    (FontSet::bundled(), Metadata::bravura().unwrap())
}

fn read(path: &Path) -> Score {
    let data = std::fs::read(path).unwrap();
    oxiguitar_formats::read(&data)
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
        .score
}

fn all_tracks() -> Options {
    Options {
        tracks: (0..64).collect(),
        ..Options::default()
    }
}

/// Problems with a page: items off the page, glyphs the font lacks.
fn page_problems(page: &DisplayList, fonts: &FontSet, slack: f32) -> Vec<String> {
    let mut out = Vec::new();
    for item in &page.items {
        let b = match item {
            Item::Glyphs(run) => {
                if run.glyphs.iter().any(|g| g.id == 0) {
                    let font = fonts.get(run.font).map_or("?", |f| f.name());
                    out.push(format!("missing glyph in {font}: {:?}", run.text));
                }
                // The origin is where the glyph is placed.
                oxiguitar_display::Rect::new(run.origin.x, run.origin.y, 0.0, 0.0)
            }
            other => other.bounds(),
        };
        if b.x < -slack
            || b.y < -slack
            || b.right() > page.width + slack
            || b.bottom() > page.height + slack
        {
            out.push(format!("off the page: {b:?}"));
        }
    }
    out
}

fn check(name: &str, e: &Engraved, fonts: &FontSet) {
    assert!(!e.pages.is_empty(), "{name}: no pages");
    for (n, page) in e.pages.iter().enumerate() {
        let problems = page_problems(page, fonts, 1.0);
        assert!(
            problems.is_empty(),
            "{name}, page {}: {}",
            n + 1,
            problems.join("; ")
        );
    }
}

#[test]
fn public_fixtures_engrave() {
    let (fonts, meta) = setup();
    for ext in ["gp3", "gp4", "gp5", "gpx", "gp"] {
        let path = root().join(format!("fixtures/gp/oxiguitar-test-song.{ext}"));
        let score = read(&path);
        let e = engrave(&score, &fonts, &meta, &Options::default());
        check(ext, &e, &fonts);
        assert_eq!(e.pages.len(), 1, "{ext}");
        assert_eq!(e.systems, 1, "{ext}: three bars fit on one line");
        // The title is on the page as text, so a PDF of it is searchable.
        let has_title = e.pages[0].items.iter().any(|i| match i {
            Item::Glyphs(r) => r.text == score.info.title,
            _ => false,
        });
        assert!(has_title, "{ext}");
    }
}

#[test]
fn engraving_is_deterministic() {
    let (fonts, meta) = setup();
    let score = read(&root().join("fixtures/gp/oxiguitar-test-song.gp"));
    let a = engrave(&score, &fonts, &meta, &all_tracks());
    let b = engrave(&score, &fonts, &meta, &all_tracks());
    assert_eq!(a.pages, b.pages);
}

#[test]
fn tablature_only_shows_rhythm() {
    let (fonts, meta) = setup();
    let mut score = read(&root().join("fixtures/gp/oxiguitar-test-song.gp5"));
    let both = engrave(&score, &fonts, &meta, &Options::default());
    for staff in &mut score.tracks[0].staves {
        staff.show_standard = false;
    }
    let tab = engrave(&score, &fonts, &meta, &Options::default());
    check("tab only", &tab, &fonts);
    let lines = |e: &Engraved| {
        e.pages[0]
            .items
            .iter()
            .filter(|i| matches!(i, Item::Line { .. }))
            .count()
    };
    // Five staff lines fewer, but stems are still drawn under the tab.
    assert!(lines(&tab) < lines(&both));
    let stems = tab.pages[0]
        .items
        .iter()
        .filter(|i| matches!(i, Item::Line { from, to, .. } if from.x == to.x && to.y > from.y))
        .count();
    assert!(stems > 10, "{stems} vertical lines");
}

#[test]
fn many_bars_break_into_systems_and_pages() {
    let (fonts, meta) = setup();
    let mut score = Score::new();
    let bars = 300;
    let mut staff = Staff {
        tuning: Staff::standard_guitar_tuning(),
        ..Staff::default()
    };
    for i in 0..bars {
        score.master_bars.push(MasterBar::default());
        let beats = (0..8)
            .map(|k| Beat {
                kind: BeatKind::Notes,
                duration: Duration::new(NoteValue::Eighth),
                notes: vec![Note::fretted(1 + (k + i) as u8 % 6, (k * 3 + i) as u8 % 15)],
                ..Beat::default()
            })
            .collect();
        staff.bars.push(Bar {
            voices: vec![Voice { beats }],
            ..Bar::default()
        });
    }
    let track = Track {
        staves: vec![staff],
        ..Track::default()
    };
    score.tracks = vec![track; 8];
    let started = std::time::Instant::now();
    let e = engrave(&score, &fonts, &meta, &all_tracks());
    let took = started.elapsed();
    check("300 bars × 8 tracks", &e, &fonts);
    assert!(e.systems >= 50, "{} systems", e.systems);
    assert!(e.pages.len() > 10);
    // The M1 budget is one second to open such a file; engraving is a part.
    if !cfg!(debug_assertions) {
        assert!(took.as_secs_f32() < 0.5, "{took:?}");
    }
}

/// Damaged or unusual scores still engrave, without panicking.
#[test]
fn odd_scores_do_not_panic() {
    let (fonts, meta) = setup();
    let check_score = |name: &str, score: &Score| {
        for options in [Options::default(), all_tracks()] {
            let e = engrave(score, &fonts, &meta, &options);
            for page in &e.pages {
                let missing: Vec<String> = page_problems(page, &fonts, f32::INFINITY);
                assert!(missing.is_empty(), "{name}: {missing:?}");
            }
        }
    };
    check_score("empty", &Score::new());

    let mut score = read(&root().join("fixtures/gp/oxiguitar-test-song.gp5"));
    check_score("fixture", &score);

    // Fewer bars in the staff than master bars; a track with no staves.
    score.tracks[0].staves[0].bars.truncate(1);
    score.tracks.push(Track::default());
    check_score("missing bars", &score);

    // Notes on strings that do not exist, extreme frets, odd durations.
    let staff = &mut score.tracks[0].staves[0];
    let voice = Voice {
        beats: vec![
            Beat {
                notes: vec![
                    Note::fretted(9, 255),
                    Note::fretted(0, 0),
                    Note::fretted(1, 99),
                ],
                duration: Duration {
                    value: NoteValue::TwoFiftySixth,
                    dots: 9,
                    tuplets: vec![Tuplet {
                        notes: 13,
                        in_time_of: 0,
                    }],
                },
                ..Beat::default()
            },
            Beat {
                notes: vec![Note {
                    place: NotePlace::Pitched(Pitch(127)),
                    ..Note::fretted(1, 0)
                }],
                duration: Duration::new(NoteValue::DoubleWhole),
                ..Beat::default()
            },
        ],
    };
    staff.bars[0].voices = vec![voice.clone(), Voice::default(), voice];
    score.master_bars[0].time_signature = TimeSignature {
        numerator: 0,
        denominator: 0,
    };
    score.master_bars[1].key_signature.fifths = -100;
    score.master_bars[0].alternate_endings = u16::MAX;
    score.master_bars[0].repeat_end = Some(255);
    check_score("damaged", &score);

    let tiny = Options {
        page_width: 1.0,
        page_height: 1.0,
        margin: 100.0,
        staff_space: 0.0,
        ..Options::default()
    };
    let _ = engrave(&score, &fonts, &meta, &tiny);
}

fn corpus_files() -> Vec<PathBuf> {
    let dir = root().join("corpus/files");
    let mut v = Vec::new();
    for sub in ["tabs", "scores"] {
        if let Ok(d) = std::fs::read_dir(dir.join(sub)) {
            v.extend(d.filter_map(|e| Some(e.ok()?.path())));
        }
    }
    v.sort();
    v
}

/// Every Guitar Pro file of the corpus engraves with every track shown,
/// everything on the page and every glyph in the fonts.
#[test]
fn corpus_engraves() {
    let files = corpus_files();
    if files.is_empty() {
        eprintln!("corpus/files not present; skipped");
        return;
    }
    let (fonts, meta) = setup();
    let mut count = 0;
    let mut failures = Vec::new();
    let mut slowest = (0.0_f32, String::new());
    for path in files {
        let Ok(data) = std::fs::read(&path) else {
            continue;
        };
        let Ok(imported) = oxiguitar_formats::read(&data) else {
            continue;
        };
        let name = path
            .file_name()
            .map_or(String::new(), |n| n.to_string_lossy().into_owned());
        let started = std::time::Instant::now();
        let e = engrave(&imported.score, &fonts, &meta, &all_tracks());
        let ms = started.elapsed().as_secs_f32() * 1000.0;
        if ms > slowest.0 {
            slowest = (ms, name.clone());
        }
        for (n, page) in e.pages.iter().enumerate() {
            let problems = page_problems(page, &fonts, 1.0);
            if let Some(first) = problems.first() {
                failures.push(format!(
                    "{name}, page {}: {} problem(s), first: {first}",
                    n + 1,
                    problems.len()
                ));
            }
        }
        count += 1;
    }
    eprintln!(
        "{count} corpus files engraved; slowest {:.1} ms ({})",
        slowest.0, slowest.1
    );
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    assert!(count >= 100, "{count}");
}
