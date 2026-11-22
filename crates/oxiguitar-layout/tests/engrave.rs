// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Engraving tests on the public fixtures, on damaged scores and on the test
//! corpus, file by file (`OXIGUITAR_CORPUS`, or `corpus/files/`; skipped
//! quietly without one unless `OXIGUITAR_CORPUS_REQUIRED` is set).
//!
//! With the corpus in place:
//! `cargo test --release -p oxiguitar-layout --test engrave -- --nocapture`

#[path = "../../../tests/support/corpus.rs"]
mod corpus;
#[path = "../../../tests/support/guitar_pro.rs"]
mod guitar_pro;

use std::path::{Path, PathBuf};

use oxiguitar_display::smufl::Metadata;
use oxiguitar_display::{DisplayList, FontSet, Item};
use oxiguitar_layout::{Engraved, Layout, Notation, Options, Paper, engrave};
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
    oxiguitar_plugin_test::released::guitar_pro::read(&data)
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
                // From the origin to the end of the last glyph's advance.
                let width = run.glyphs.last().map_or(0.0, |g| g.x + g.advance);
                oxiguitar_display::Rect::new(run.origin.x, run.origin.y, width, 0.0)
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
    // Five staff lines fewer, but the rhythm is drawn around the tab.
    assert!(lines(&tab) < lines(&both));

    // The tab staff: the long horizontal lines of the one system.
    let page = &tab.pages[0];
    let tab_lines: Vec<f32> = page
        .items
        .iter()
        .filter_map(|i| match i {
            Item::Line { from, to, .. } if from.y == to.y && to.x - from.x > 200.0 => Some(from.y),
            _ => None,
        })
        .collect();
    let top = tab_lines.iter().copied().fold(f32::INFINITY, f32::min);
    let bottom = tab_lines.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    assert_eq!(tab_lines.len(), 6, "{tab_lines:?}");
    // Stems by voice number: voice 1 above the tab, voice 2 below.
    let stem_width = meta.engraving_defaults.stem_thickness * Options::default().staff_space;
    let (mut up, mut down) = (0, 0);
    for i in &page.items {
        if let Item::Line {
            from, to, width, ..
        } = i
            && from.x == to.x
            && (*width - stem_width).abs() < 0.01
        {
            if from.y.min(to.y) < top - 5.0 {
                up += 1;
            }
            if from.y.max(to.y) > bottom + 5.0 {
                down += 1;
            }
        }
    }
    let voice_1 = score.tracks[0].staves[0]
        .bars
        .iter()
        .flat_map(|b| b.voices.first())
        .flat_map(|v| &v.beats)
        .filter(|b| !b.notes.is_empty() && b.duration.value >= NoteValue::Half && b.grace.is_none())
        .count();
    assert_eq!(up, voice_1, "voice 1 stems above the tab");
    assert!(down > 0, "voice 2 stems below the tab");
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

/// Every Guitar Pro file of the test corpus (`OXIGUITAR_CORPUS`, or
/// `corpus/files/`; see `tests/support/corpus.rs`) engraves in every layout
/// with every track shown, everything on the sheet and every glyph in the
/// fonts.
#[test]
fn corpus_engraves() {
    let (fonts, meta) = setup();
    let mut count = 0;
    let mut slowest = (0.0_f32, String::new());
    corpus::check("engraving", &root().join("corpus/files"), |path, data| {
        let Some(score) = guitar_pro::read(data)? else {
            return Ok(corpus::Outcome::NotTaken);
        };
        let mut failures = Vec::new();
        let name = path
            .file_name()
            .map_or(String::new(), |n| n.to_string_lossy().into_owned());
        for layout in Layout::ALL {
            let options = Options {
                layout,
                screen_width: 900.0,
                ..all_tracks()
            };
            let started = std::time::Instant::now();
            let e = engrave(&score, &fonts, &meta, &options);
            let ms = started.elapsed().as_secs_f32() * 1000.0;
            if ms > slowest.0 {
                slowest = (ms, format!("{name}, {}", layout.name()));
            }
            if !layout.has_pages() && e.pages.len() != 1 {
                failures.push(format!(
                    "{name}, {}: {} sheets",
                    layout.name(),
                    e.pages.len()
                ));
            }
            for (n, page) in e.pages.iter().enumerate() {
                let problems = page_problems(page, &fonts, 1.0);
                if let Some(first) = problems.first() {
                    failures.push(format!(
                        "{name}, {}, page {}: {} problem(s), first: {first}",
                        layout.name(),
                        n + 1,
                        problems.len()
                    ));
                }
            }
        }
        count += 1;
        if failures.is_empty() {
            Ok(corpus::Outcome::Passed)
        } else {
            Err(failures.join("; "))
        }
    });
    eprintln!(
        "{count} corpus files engraved; slowest {:.1} ms ({})",
        slowest.0, slowest.1
    );
}

/// Every text of a page, in drawing order.
fn texts(page: &DisplayList) -> Vec<String> {
    page.items
        .iter()
        .filter_map(|i| match i {
            Item::Glyphs(run) => Some(run.text.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn effects_lyrics_and_diagrams_are_drawn() {
    let (fonts, meta) = setup();
    // The GP7 test song: a hammer-on, a full bend, lyrics "la la la", one
    // chord diagram.
    let score = read(&root().join("fixtures/gp/oxiguitar-test-song.gp"));
    let e = engrave(&score, &fonts, &meta, &Options::default());
    check("gp", &e, &fonts);
    let t = texts(&e.pages[0]);
    assert!(
        t.iter().any(|s| s == "H" || s == "P"),
        "hammer-on letter: {t:?}"
    );
    assert!(t.iter().any(|s| s == "full"), "bend amount: {t:?}");
    assert_eq!(t.iter().filter(|s| *s == "la").count(), 3, "lyrics: {t:?}");
    let chord = &score.tracks[0].chords[0].name;
    assert_eq!(
        t.iter().filter(|s| *s == chord).count(),
        2,
        "the chord name over the staff and over its diagram in the header"
    );
}

#[test]
fn chord_names_with_music_signs_have_every_glyph() {
    let (fonts, meta) = setup();
    let mut score = read(&root().join("fixtures/gp/oxiguitar-test-song.gp"));
    score.tracks[0].chords[0].name = "F♯m7♭5 B♮".to_owned();
    let e = engrave(&score, &fonts, &meta, &Options::default());
    check("music signs", &e, &fonts);
}

#[test]
fn grace_notes_come_small_before_their_note() {
    let (fonts, meta) = setup();
    let mut score = Score::new();
    score.master_bars.push(MasterBar::default());
    let mut grace = Beat {
        notes: vec![Note::fretted(2, 5)],
        duration: Duration::new(NoteValue::Sixteenth),
        grace: Some(oxiguitar_model::GraceKind::BeforeBeat),
        ..Beat::default()
    };
    grace.kind = BeatKind::Notes;
    let main = Beat {
        notes: vec![Note::fretted(2, 7)],
        duration: Duration::new(NoteValue::Whole),
        ..Beat::default()
    };
    let staff = Staff {
        tuning: Staff::standard_guitar_tuning(),
        bars: vec![Bar {
            voices: vec![Voice {
                beats: vec![grace, main],
            }],
            ..Bar::default()
        }],
        ..Staff::default()
    };
    score.tracks.push(Track {
        staves: vec![staff],
        ..Track::default()
    });
    let e = engrave(&score, &fonts, &meta, &Options::default());
    check("grace", &e, &fonts);
    let runs: Vec<&oxiguitar_display::GlyphRun> = e.pages[0]
        .items
        .iter()
        .filter_map(|i| match i {
            Item::Glyphs(r) if r.text == "5" || r.text == "7" => Some(r),
            _ => None,
        })
        .collect();
    let five = runs
        .iter()
        .find(|r| r.text == "5")
        .expect("grace fret number");
    let seven = runs
        .iter()
        .find(|r| r.text == "7")
        .expect("main fret number");
    assert!(five.size < seven.size, "the grace number is smaller");
    assert!(five.origin.x < seven.origin.x, "and comes first");
}

/// A one-track score in G major written without a key signature, as most
/// tabs are: each bar G B D F♯, the first beat marked forte.
fn tune_in_g(bars: usize) -> Score {
    let mut score = Score::new();
    let mut staff = Staff::default();
    for _ in 0..bars {
        score.master_bars.push(MasterBar::default());
        let beats = [67, 71, 74, 66]
            .into_iter()
            .map(|p| Beat {
                notes: vec![Note {
                    place: NotePlace::Pitched(Pitch(p)),
                    ..Note::fretted(1, 0)
                }],
                duration: Duration::new(NoteValue::Quarter),
                ..Beat::default()
            })
            .collect();
        staff.bars.push(Bar {
            voices: vec![Voice { beats }],
            ..Bar::default()
        });
    }
    if let Some(beat) = staff.bars[0].voices[0].beats.first_mut() {
        beat.effects.dynamic = Some(oxiguitar_model::Dynamic::F);
    }
    score.tracks.push(Track {
        staves: vec![staff],
        ..Track::default()
    });
    score
}

#[test]
fn a_plugin_s_mark_is_drawn_at_its_beat() {
    let (fonts, meta) = setup();
    let mut score = tune_in_g(2);
    let plain = engrave(&score, &fonts, &meta, &Options::default());
    assert!(!plain.pages.iter().flat_map(texts).any(|t| t == "T!"));
    score.tracks[0].staves[0].bars[1].voices[0].beats[2]
        .effects
        .marks
        .insert("oxiguitar.test".into(), "T!".into());
    let e = engrave(&score, &fonts, &meta, &Options::default());
    check("mark", &e, &fonts);
    let drawn: Vec<_> = e
        .pages
        .iter()
        .flat_map(|p| &p.items)
        .filter_map(|i| match i {
            Item::Glyphs(run) if run.text == "T!" => Some(run.origin),
            _ => None,
        })
        .collect();
    assert_eq!(drawn.len(), 1, "the mark, once");
    // Over its own beat: the third of the second bar.
    let beats = &e.hits.systems[0].staves[0].bars[1].beats;
    let (here, next) = (beats[2].x, beats[3].x);
    assert!(
        drawn[0].x >= here - 4.0 && drawn[0].x < next,
        "{} in {here}..{next}",
        drawn[0].x
    );
    // And above the staff.
    assert!(drawn[0].y < e.hits.systems[0].staves[0].top);
}

#[test]
fn others_drawings_stand_in_a_lane_of_their_own_over_their_beat() {
    use oxiguitar_display::{Paint, Point};
    use oxiguitar_layout::{Anchor, Drawing, DrawnAt, Shape};
    let (fonts, meta) = setup();
    let score = tune_in_g(2);
    let plain = engrave(&score, &fonts, &meta, &Options::default());
    // A box 3 by 2 staff spaces: a line across its bottom and a word.
    let drawing = Drawing {
        tag: 7,
        width: 3.0,
        height: 2.0,
        shapes: vec![
            Shape::Line {
                from: Point::new(0.0, 2.0),
                to: Point::new(3.0, 2.0),
                width: 0.1,
                paint: Paint::Accent,
            },
            Shape::Text {
                text: "Hey".into(),
                at: Point::new(1.5, 1.5),
                size: 1.6,
                music: false,
                anchor: Anchor::Middle,
                paint: Paint::Ink,
            },
        ],
        run: None,
    };
    let mut options = Options::default();
    options.drawings.insert(
        DrawnAt {
            track: 0,
            staff: 0,
            bar: 1,
            beat: Some((0, 2)),
        },
        vec![drawing],
    );
    let e = engrave(&score, &fonts, &meta, &options);
    check("drawing", &e, &fonts);
    let sp = options.staff_space;
    let lines: Vec<(Point, Point)> = e
        .pages
        .iter()
        .flat_map(|p| &p.items)
        .filter_map(|i| match i {
            Item::Line {
                from,
                to,
                paint: Paint::Accent,
                ..
            } => Some((*from, *to)),
            _ => None,
        })
        .collect();
    assert_eq!(lines.len(), 1, "the line, once");
    let (from, to) = lines[0];
    assert!(
        (to.x - from.x - 3.0 * sp).abs() < 0.01,
        "3 staff spaces wide"
    );
    // Over its beat, above the staff.
    let staff = &e.hits.systems[0].staves[0];
    let beats = &staff.bars[1].beats;
    assert!(
        from.x >= beats[2].x - sp && from.x < beats[3].x,
        "{} vs {}",
        from.x,
        beats[2].x
    );
    assert!(to.y < staff.top);
    assert!(e.pages.iter().flat_map(texts).any(|t| t == "Hey"));
    // The system makes room for it: the staff is lower than without.
    assert!(staff.top > plain.hits.systems[0].staves[0].top + 1.5 * sp);
    // A click on it finds it, its tag and where in its box; one beside it
    // doesn't.
    let found = e
        .hits
        .drawn_at(0, from.x + 1.5 * sp, to.y - 0.5 * sp)
        .expect("the drawing is there");
    assert_eq!(found.tag, 7);
    assert_eq!(
        found.at,
        DrawnAt {
            track: 0,
            staff: 0,
            bar: 1,
            beat: Some((0, 2)),
        }
    );
    assert!(
        (found.x - 1.5).abs() < 0.01 && (found.y - 1.5).abs() < 0.01,
        "{found:?}"
    );
    assert!(
        e.hits
            .drawn_at(0, from.x + 1.5 * sp, staff.bottom + sp)
            .is_none()
    );
    assert!(
        plain
            .hits
            .drawn_at(0, from.x + 1.5 * sp, to.y - 0.5 * sp)
            .is_none()
    );
}

/// How many times `c` is drawn on every page.
fn count_char(e: &Engraved, c: char) -> usize {
    e.pages
        .iter()
        .flat_map(texts)
        .map(|t| t.chars().filter(|x| *x == c).count())
        .sum()
}

#[test]
fn the_files_key_is_shown_as_it_is() {
    let (fonts, meta) = setup();
    let mut score = tune_in_g(12);
    let sharp = '\u{E262}';
    // The file says C major: a sharp before every F♯, and nothing is
    // inferred (that is a tool the user picks, in oxiguitar-tools).
    let e = engrave(&score, &fonts, &meta, &Options::default());
    assert_eq!(count_char(&e, sharp), 12);
    // In G: one sharp in the signature of each system, none on the notes.
    for mb in &mut score.master_bars {
        mb.key_signature.fifths = 1;
    }
    let e = engrave(&score, &fonts, &meta, &Options::default());
    check("key of G", &e, &fonts);
    assert_eq!(count_char(&e, sharp), e.systems);
}

#[test]
fn dynamics_can_be_hidden() {
    let (fonts, meta) = setup();
    let score = tune_in_g(2);
    let forte = '\u{E522}';
    let e = engrave(&score, &fonts, &meta, &Options::default());
    assert_eq!(count_char(&e, forte), 1);
    let hidden = Options {
        dynamics: false,
        ..Options::default()
    };
    let e = engrave(&score, &fonts, &meta, &hidden);
    assert_eq!(count_char(&e, forte), 0);
}

#[test]
fn any_tuplet_grouping_is_drawn() {
    let (fonts, meta) = setup();
    // One 4/4 bar: five eighths in the time of three, then six sixteenths
    // in the time of four, then a quarter rest's worth of plain eighths.
    let beat = |value, tuplet: Option<Tuplet>| Beat {
        notes: vec![Note {
            place: NotePlace::Pitched(Pitch(67)),
            ..Note::fretted(1, 0)
        }],
        duration: Duration {
            tuplets: tuplet.into_iter().collect(),
            ..Duration::new(value)
        },
        ..Beat::default()
    };
    let five = Tuplet {
        notes: 5,
        in_time_of: 3,
    };
    let six = Tuplet {
        notes: 6,
        in_time_of: 4,
    };
    let mut beats: Vec<Beat> = (0..5)
        .map(|_| beat(NoteValue::Eighth, Some(five)))
        .collect();
    beats.extend((0..6).map(|_| beat(NoteValue::Sixteenth, Some(six))));
    beats.extend((0..2).map(|_| beat(NoteValue::Eighth, None)));
    let mut score = Score::new();
    score.master_bars.push(MasterBar::default());
    score.tracks.push(Track {
        staves: vec![Staff {
            bars: vec![Bar {
                voices: vec![Voice { beats }],
                ..Bar::default()
            }],
            ..Staff::default()
        }],
        ..Track::default()
    });
    let e = engrave(&score, &fonts, &meta, &Options::default());
    check("tuplets", &e, &fonts);
    let colon = '\u{E88A}';
    let (five_glyph, three_glyph, six_glyph) = ('\u{E885}', '\u{E883}', '\u{E886}');
    assert_eq!(
        count_char(&e, colon),
        1,
        "5:3 shows its ratio, 6:4 does not"
    );
    assert_eq!(count_char(&e, five_glyph), 1);
    assert_eq!(count_char(&e, three_glyph), 1);
    assert_eq!(count_char(&e, six_glyph), 1);
}

/// The same bars in each of the four layouts (VIEW-3).
#[test]
fn the_four_layouts() {
    let (fonts, meta) = setup();
    let mut score = tune_in_g(150);
    score.info.title = "Four layouts".to_owned();
    let with = |layout, screen_width| Options {
        layout,
        screen_width,
        ..Options::default()
    };

    let page = engrave(&score, &fonts, &meta, &with(Layout::Page, 0.0));
    check("page", &page, &fonts);
    assert!(page.pages.len() > 1, "150 bars need more than one page");
    let a4 = Options::default();
    assert!(
        page.pages
            .iter()
            .all(|p| p.width == a4.page_width && p.height == a4.page_height)
    );

    // Parchment: the Page layout's systems, on one sheet as wide as the page.
    let parchment = engrave(&score, &fonts, &meta, &with(Layout::Parchment, 0.0));
    check("parchment", &parchment, &fonts);
    assert_eq!(parchment.pages.len(), 1);
    assert_eq!(parchment.systems, page.systems);
    assert_eq!(parchment.pages[0].width, a4.page_width);
    assert!(parchment.pages[0].height > a4.page_height);

    // Vertical: systems fill the window's width; a narrower window takes
    // more systems.
    let wide = engrave(&score, &fonts, &meta, &with(Layout::Vertical, 1400.0));
    let narrow = engrave(&score, &fonts, &meta, &with(Layout::Vertical, 400.0));
    check("vertical, wide", &wide, &fonts);
    check("vertical, narrow", &narrow, &fonts);
    assert_eq!(wide.pages.len(), 1);
    assert_eq!(wide.pages[0].width, 1400.0);
    assert!(
        narrow.systems > wide.systems,
        "{} vs {}",
        narrow.systems,
        wide.systems
    );

    // Horizontal: one line as long as the music, no header.
    let line = engrave(&score, &fonts, &meta, &with(Layout::Horizontal, 0.0));
    check("horizontal", &line, &fonts);
    assert_eq!((line.pages.len(), line.systems), (1, 1));
    let sheet = &line.pages[0];
    assert!(sheet.width > 10.0 * a4.page_width, "{}", sheet.width);
    assert!(sheet.height < a4.page_height / 2.0, "{}", sheet.height);
    assert!(!texts(sheet).contains(&score.info.title));
    assert!(texts(&page.pages[0]).contains(&score.info.title));
}

#[test]
fn layouts_and_paper_have_names() {
    for l in Layout::ALL {
        assert_eq!(Layout::from_name(&l.name().to_lowercase()), Some(l));
    }
    assert_eq!(Layout::from_name("scroll"), None);
    let letter = Options::default().with_paper(Paper::Letter);
    assert_eq!((letter.page_width, letter.page_height), (612.0, 792.0));
    assert_eq!(Paper::from_name("a4"), Some(Paper::A4));
}

/// Glyphs of the music font on the first page.
fn glyphs_on_first_page(e: &Engraved) -> String {
    e.pages[0]
        .items
        .iter()
        .filter_map(|i| match i {
            Item::Glyphs(r) => Some(r.text.clone()),
            _ => None,
        })
        .collect()
}

/// A keyboard track is shown on a grand staff, without tablature unless the
/// user asks; only fretted instruments show tab by default (Mateus,
/// 16 Oct 2026).
#[test]
fn keyboards_get_a_grand_staff_and_tab_only_when_asked() {
    let (fonts, meta) = setup();
    let (brace, g_clef, f_clef, tab_clef) = ('\u{E000}', '\u{E050}', '\u{E062}', '\u{E06D}');
    let mut score = tune_in_g(8);
    score.tracks[0].staves[0].tuning = Staff::standard_guitar_tuning();
    // A low left hand under the tune.
    for bar in &mut score.tracks[0].staves[0].bars {
        if let Some(b) = bar.voices[0].beats.first_mut() {
            b.notes.push(Note {
                place: NotePlace::Pitched(Pitch(36)),
                ..Note::fretted(1, 0)
            });
        }
        bar.clef = oxiguitar_model::Clef::Treble;
    }
    score.tracks[0].instrument.midi_program = 0; // Acoustic Grand Piano

    let e = engrave(&score, &fonts, &meta, &Options::default());
    check("piano", &e, &fonts);
    let g = glyphs_on_first_page(&e);
    assert!(g.contains(brace), "a brace joins the two staves");
    assert!(
        g.contains(g_clef) && g.contains(f_clef),
        "treble and bass clefs"
    );
    assert!(!g.contains(tab_clef), "no tablature for a piano by default");

    let mut asked = Options::default();
    asked.notation.insert(
        0,
        Notation {
            standard: true,
            tablature: true,
        },
    );
    let e = engrave(&score, &fonts, &meta, &asked);
    check("piano with tab", &e, &fonts);
    assert!(
        glyphs_on_first_page(&e).contains(tab_clef),
        "tab when asked for"
    );

    // A fretless bass: notation only, unless asked.
    score.tracks[0].instrument.midi_program = 35;
    let e = engrave(&score, &fonts, &meta, &Options::default());
    let g = glyphs_on_first_page(&e);
    assert!(!g.contains(tab_clef) && !g.contains(brace));
    // A guitar keeps its tab.
    score.tracks[0].instrument.midi_program = 25;
    let e = engrave(&score, &fonts, &meta, &Options::default());
    assert!(glyphs_on_first_page(&e).contains(tab_clef));
    let mut hidden = Options::default();
    hidden.notation.insert(
        0,
        Notation {
            standard: true,
            tablature: false,
        },
    );
    let e = engrave(&score, &fonts, &meta, &hidden);
    assert!(
        !glyphs_on_first_page(&e).contains(tab_clef),
        "tab hidden when asked"
    );
    // Tab only: no staff clef, the tab with its rhythm.
    let mut tab_only = Options::default();
    tab_only.notation.insert(
        0,
        Notation {
            standard: false,
            tablature: true,
        },
    );
    let g = glyphs_on_first_page(&engrave(&score, &fonts, &meta, &tab_only));
    assert!(g.contains(tab_clef) && !g.contains('\u{E052}'), "tab only");
    // A piano shown as tab only has no grand staff.
    score.tracks[0].instrument.midi_program = 0;
    let g = glyphs_on_first_page(&engrave(&score, &fonts, &meta, &tab_only));
    assert!(
        g.contains(tab_clef) && !g.contains(brace),
        "piano as tab only"
    );
}

/// Bars that don't add up are marked in translucent red on the staves they
/// belong to, and nowhere else; the same score without marks has none (a
/// PDF shows what the file says).
#[test]
fn marked_bars_are_laid_in_red() {
    use oxiguitar_display::Paint;
    let (fonts, meta) = setup();
    let score = read(&root().join("fixtures/gp/oxiguitar-test-song.gp5"));
    let warnings = |e: &Engraved| -> Vec<oxiguitar_display::Rect> {
        e.pages
            .iter()
            .flat_map(|p| &p.items)
            .filter_map(|i| match i {
                Item::Rect {
                    rect,
                    paint: Paint::Warning,
                } => Some(*rect),
                _ => None,
            })
            .collect()
    };
    let plain = engrave(&score, &fonts, &meta, &Options::default());
    assert!(warnings(&plain).is_empty());
    let options = Options {
        marked: [(0, 0, 1), (5, 0, 1)].into_iter().collect(),
        ..Options::default()
    };
    let marked = engrave(&score, &fonts, &meta, &options);
    let rects = warnings(&marked);
    assert_eq!(rects.len(), 1, "only the shown track's bar");
    // The mark covers the bar the map gives for bar 2.
    let bar = marked.hits.systems[0].staves[0]
        .bars
        .iter()
        .find(|b| b.bar == 1)
        .expect("bar 2 in the first system");
    assert!((rects[0].x - bar.start).abs() < 0.01 && (rects[0].right() - bar.end).abs() < 0.01);
    // Drawn under the notation: nothing that overlaps it comes before it.
    let items = &marked.pages[0].items;
    let at = items
        .iter()
        .position(|i| {
            matches!(
                i,
                Item::Rect {
                    paint: Paint::Warning,
                    ..
                }
            )
        })
        .expect("the mark");
    let r = rects[0];
    let overlapping_before = items[..at].iter().filter(|i| {
        let b = i.bounds();
        b.x < r.right() && r.x < b.right() && b.y < r.bottom() && r.y < b.bottom()
    });
    assert_eq!(overlapping_before.count(), 0);
}

/// The editor greys out every voice but its own (Mateus, 20 Oct 2026); a
/// print greys out none.
#[test]
fn other_voices_are_greyed_out_for_the_editor() {
    use oxiguitar_display::Paint;
    let (fonts, meta) = setup();
    let score = read(&root().join("fixtures/reference/two-voices.gp"));
    let muted = |active: Option<usize>| -> usize {
        let e = engrave(
            &score,
            &fonts,
            &meta,
            &Options {
                active_voice: active,
                ..Options::default()
            },
        );
        e.pages
            .iter()
            .flat_map(|p| &p.items)
            .filter(|i| match i {
                Item::Glyphs(r) => r.paint == Paint::Muted,
                Item::Line { paint, .. } | Item::Rect { paint, .. } => *paint == Paint::Muted,
                Item::Path { fill, stroke, .. } => {
                    *fill == Some(Paint::Muted) || stroke.is_some_and(|(_, p)| p == Paint::Muted)
                }
                Item::Image { .. } => false,
            })
            .count()
    };
    let none = muted(None);
    let first = muted(Some(0));
    let second = muted(Some(1));
    // Voice 1 holds most of the notes, so editing voice 2 greys out more.
    assert!(first > none, "{first} > {none}");
    assert!(second > first, "{second} > {first}");
    // A voice the song doesn't use greys out both.
    assert!(muted(Some(3)) > second);
}

/// A guitar bar of four quarter notes on string 2, frets 1 to 4, each beat
/// changed by `mark`.
fn marked_bar(mark: impl Fn(usize, &mut Beat)) -> Bar {
    let beats = (0..4)
        .map(|k| {
            let mut b = Beat {
                duration: Duration::new(NoteValue::Quarter),
                notes: vec![Note::fretted(2, 1 + k as u8)],
                ..Beat::default()
            };
            mark(k, &mut b);
            b
        })
        .collect();
    Bar {
        voices: vec![Voice { beats }],
        ..Bar::default()
    }
}

/// Every mark of the rest of the palette (v0.17), a bar or two each.
fn palette_song() -> Score {
    use oxiguitar_model::{
        Barre, Beaming, Fade, Fermata, FermataKind, Golpe, Hairpin, Ornament, Ottava, Pedal,
        Rasgueado, Ratio, Simile, StrokeDirection, Wah,
    };
    let mut staff = Staff {
        tuning: Staff::standard_guitar_tuning(),
        ..Staff::default()
    };
    staff.bars = vec![
        marked_bar(|k, b| {
            b.slur = k < 2;
            b.hairpin = Some(Hairpin::Crescendo);
            b.effects.pedal = match k {
                0 => Some(Pedal::Down),
                3 => Some(Pedal::Up),
                _ => None,
            };
        }),
        marked_bar(|k, b| {
            b.ottava = Some(Ottava::Alta8);
            if k == 0 {
                b.notes[0].effects.ornament = Some(Ornament::Turn);
                b.notes[0].effects.left_finger = Some(1);
                b.notes[0].effects.right_finger = Some(1);
                b.notes[0].effects.string_number = true;
            }
        }),
        marked_bar(|k, b| match k {
            0 => b.effects.fade = Some(Fade::Out),
            1 => {
                b.effects.wah = Some(Wah::Closed);
                b.effects.golpe = Some(Golpe::Thumb);
            }
            2 => {
                b.effects.rasgueado = Some(Rasgueado::Ii);
                b.effects.pick_scrape = Some(StrokeDirection::Up);
            }
            _ => b.effects.dead_slap = true,
        }),
        marked_bar(|k, b| {
            b.barre = Some(Barre {
                fret: 5,
                half: false,
            });
            if k == 0 {
                b.notes.push(Note::fretted(3, 5));
                b.effects.arpeggio = Some(StrokeDirection::Up);
            }
        }),
        marked_bar(|_, b| b.slash = true),
        marked_bar(|_, _| {}),
        marked_bar(|_, _| {}),
        marked_bar(|_, _| {}),
        marked_bar(|_, b| {
            b.duration = Duration::new(NoteValue::Eighth);
            b.beaming = Beaming::Break;
        }),
    ];
    staff.bars[5].simile = Some(Simile::Simple);
    staff.bars[6].simile = Some(Simile::FirstOfDouble);
    staff.bars[7].simile = Some(Simile::SecondOfDouble);
    let mut score = Score::new();
    score.master_bars = vec![MasterBar::default(); staff.bars.len()];
    score.master_bars[8].time_signature = TimeSignature {
        numerator: 2,
        denominator: 4,
    };
    score.master_bars[0].fermatas.push(Fermata {
        position: Ratio::new(1, 4).unwrap(),
        kind: FermataKind::Long,
        length: 0.0,
    });
    score.tracks.push(Track {
        staves: vec![staff],
        ..Track::default()
    });
    score
}

#[test]
fn the_marks_of_the_rest_of_the_palette_are_drawn() {
    let (fonts, meta) = setup();
    let score = palette_song();
    for notation in [
        Notation {
            standard: true,
            tablature: true,
        },
        Notation {
            standard: false,
            tablature: true,
        },
    ] {
        let options = Options {
            notation: [(0, notation)].into_iter().collect(),
            ..Options::default()
        };
        let e = engrave(&score, &fonts, &meta, &options);
        check("palette", &e, &fonts);
        let std = notation.standard;
        // Fermata (long), wah, golpe, pedal down and up, 8va: once.
        for (c, what) in [
            ('\u{E4C6}', "long fermata"),
            ('\u{E5E5}', "closed wah"),
            ('\u{E842}', "golpe"),
            ('\u{E650}', "pedal"),
            ('\u{E655}', "pedal release"),
            ('\u{E511}', "8va"),
        ] {
            assert_eq!(count_char(&e, c), 1, "{what}, standard {std}");
        }
        // The repeat signs: on each staff.
        let staves = if std { 2 } else { 1 };
        assert_eq!(count_char(&e, '\u{E500}'), staves, "one-bar repeat");
        assert_eq!(count_char(&e, '\u{E501}'), staves, "two-bar repeat");
        // On the staff only: the turn, the fingers, slash heads.
        let staff_only = [
            ('\u{E567}', "turn"),
            ('\u{ED11}', "finger 1"),
            ('\u{ED19}', "i"),
            ('\u{E101}', "slash heads"),
        ];
        for (c, what) in staff_only {
            let n = count_char(&e, c);
            if std {
                assert!(n >= 1, "{what}");
            } else if c != '\u{E567}' {
                assert_eq!(n, 0, "{what} on a tab-only staff");
            }
        }
        let t: Vec<String> = e.pages.iter().flat_map(texts).collect();
        for want in ["rasg. ii", "dead slap", "CV"] {
            assert!(t.iter().any(|s| s == want), "{want}: {t:?}");
        }
    }
}

#[test]
fn a_key_change_at_a_line_break_is_shown_again_before_it() {
    let (fonts, meta) = setup();
    let mut score = tune_in_g(40);
    let e = engrave(&score, &fonts, &meta, &Options::default());
    let second = e.hits.systems[1].staves[0].bars[0].bar;
    // From the second system's first bar on, G major.
    for mb in score.master_bars.iter_mut().skip(second) {
        mb.key_signature.fifths = 1;
    }
    let e = engrave(&score, &fonts, &meta, &Options::default());
    assert_eq!(
        e.hits.systems[1].staves[0].bars[0].bar, second,
        "the line still breaks there"
    );
    let later = e.hits.systems.len() - 1;
    // In C, each bar's F sharp has its sharp; from the change on, one
    // sharp at the start of each later system (the F sharps are in the
    // key); and one before the break: the courtesy key signature.
    assert_eq!(count_char(&e, '\u{E262}'), second + later + 1);
}

#[test]
fn systems_hold_the_bars_set_in_design_mode() {
    let (fonts, meta) = setup();
    let mut score = tune_in_g(20);
    let first_bars = |e: &Engraved| -> Vec<usize> {
        e.hits
            .systems
            .iter()
            .map(|s| s.staves[0].bars[0].bar)
            .collect()
    };
    let automatic = first_bars(&engrave(&score, &fonts, &meta, &Options::default()));
    // One track shown: its own layout. Two bars, then three, then the
    // rest as many as fit.
    score.tracks[0].layout = vec![2, 3];
    let e = engrave(&score, &fonts, &meta, &Options::default());
    let set = first_bars(&e);
    assert_eq!(set[..3], [0, 2, 5], "{set:?} (automatic {automatic:?})");
    // The score's layout is for several tracks: not used here.
    score.tracks[0].layout.clear();
    score.layout = vec![1];
    assert_eq!(
        first_bars(&engrave(&score, &fonts, &meta, &Options::default())),
        automatic
    );
    // Not in the Vertical layout.
    score.tracks[0].layout = vec![1, 1];
    let vertical = Options {
        layout: Layout::Vertical,
        screen_width: 800.0,
        ..Options::default()
    };
    assert_ne!(
        first_bars(&engrave(&score, &fonts, &meta, &vertical))[..2],
        [0, 1]
    );
    // More bars than fit: as many as fit, and Design Mode says so in red.
    score.tracks[0].layout = vec![20];
    let design = Options {
        design: true,
        ..Options::default()
    };
    let e = engrave(&score, &fonts, &meta, &design);
    assert_eq!(first_bars(&e), automatic);
    let red = e.pages[0].items.iter().any(|i| match i {
        Item::Glyphs(run) => {
            run.text.ends_with(" bars") && run.paint == oxiguitar_display::Paint::Warning
        }
        _ => false,
    });
    assert!(red, "the count in red");
}

/// docs/design/drums.md: a drum track's tablature shows each element's
/// General MIDI key on the kit map's line (cymbals on top, the kick at the
/// bottom), and the drum staff draws the same notes as ever.
#[test]
fn drum_tablature_shows_the_kit_s_numbers() {
    let (fonts, meta) = setup();
    let mut score = Score::new();
    score.master_bars.push(MasterBar::default());
    let hit = |keys: &[u8]| Beat {
        notes: keys
            .iter()
            .map(|k| Note {
                place: NotePlace::Pitched(Pitch(*k)),
                ..Note::fretted(1, 0)
            })
            .collect(),
        duration: Duration::new(NoteValue::Quarter),
        ..Beat::default()
    };
    let beats = vec![hit(&[36, 49]), hit(&[42]), hit(&[38, 42]), hit(&[36, 42])];
    score.tracks.push(Track {
        instrument: oxiguitar_model::Instrument {
            midi_program: 0,
            percussion: true,
        },
        staves: vec![Staff {
            tuning: Vec::new(),
            bars: vec![Bar {
                voices: vec![Voice { beats }],
                ..Bar::default()
            }],
            ..Staff::default()
        }],
        ..Track::default()
    });
    let both = Options {
        notation: [(
            0,
            Notation {
                standard: true,
                tablature: true,
            },
        )]
        .into_iter()
        .collect(),
        ..Options::default()
    };
    let e = engrave(&score, &fonts, &meta, &both);
    check("drum tab", &e, &fonts);
    let staff = &e.hits.systems[0].staves[0];
    let (tab_top, gap, lines) = staff.tablature.expect("drum tablature");
    assert_eq!(lines, 6);
    let at = |text: &str| -> Vec<f32> {
        e.pages[0]
            .items
            .iter()
            .filter_map(|i| match i {
                Item::Glyphs(run) if run.text == text => Some(run.origin.y),
                _ => None,
            })
            .collect()
    };
    let line_of = |y: f32| ((y - tab_top) / gap).round() as i32 + 1;
    assert_eq!(at("42").len(), 3);
    assert!(
        at("42").iter().all(|y| line_of(*y) == 2),
        "hi-hats on line 2"
    );
    assert_eq!(
        at("49").iter().map(|y| line_of(*y)).collect::<Vec<_>>(),
        [1]
    );
    assert_eq!(
        at("38").iter().map(|y| line_of(*y)).collect::<Vec<_>>(),
        [4]
    );
    assert!(at("36").iter().all(|y| line_of(*y) == 6), "kick on line 6");
    // Standard notation alone: no numbers.
    let plain = engrave(&score, &fonts, &meta, &Options::default());
    assert!(plain.hits.systems[0].staves[0].tablature.is_none());
    assert!(!plain.pages.iter().flat_map(texts).any(|t| t == "38"));
}

#[test]
fn a_drawing_s_run_goes_on_across_bars_and_systems_to_its_point() {
    use oxiguitar_display::{Paint, Point};
    use oxiguitar_layout::{Drawing, DrawnAt, Run};
    let (fonts, meta) = setup();
    let score = tune_in_g(24);
    let plain = engrave(&score, &fonts, &meta, &Options::default());
    let at = DrawnAt {
        track: 0,
        staff: 0,
        bar: 1,
        beat: Some((0, 0)),
    };
    // A box 2 by 2 staff spaces, its run to halfway through bar 20.
    let mut options = Options::default();
    options.drawings.insert(
        at,
        vec![Drawing {
            tag: 1,
            width: 2.0,
            height: 2.0,
            shapes: Vec::new(),
            run: Some(Run {
                to_bar: 20,
                to: 0.5,
                width: 0.1,
                paint: Paint::Accent,
                dashed: false,
                hook: true,
            }),
        }],
    );
    let e = engrave(&score, &fonts, &meta, &options);
    check("run", &e, &fonts);
    let sp = options.staff_space;
    // Which system each bar is in.
    let system_of = |bar: usize| {
        e.hits
            .systems
            .iter()
            .position(|s| s.staves[0].bars.iter().any(|b| b.bar == bar))
            .unwrap()
    };
    let (first, last) = (system_of(1), system_of(20));
    assert!(last > first, "the run crosses systems");
    let lines: Vec<(Point, Point)> = e
        .pages
        .iter()
        .flat_map(|p| &p.items)
        .filter_map(|i| match i {
            Item::Line {
                from,
                to,
                paint: Paint::Accent,
                ..
            } => Some((*from, *to)),
            _ => None,
        })
        .collect();
    let across: Vec<&(Point, Point)> = lines.iter().filter(|(a, b)| a.y == b.y).collect();
    let down: Vec<&(Point, Point)> = lines.iter().filter(|(a, b)| a.x == b.x).collect();
    assert_eq!(across.len(), last - first + 1, "one piece a system");
    assert_eq!(down.len(), 1, "one hook, at its end");
    // It starts at its box's right edge.
    let drawn = &e.hits.systems[first].drawn[0];
    assert!((across[0].0.x - (drawn.rect.x + drawn.rect.width)).abs() < 0.01);
    // It ends halfway through bar 20: between its third beat and its
    // fourth's column, by the onsets.
    let bar = e.hits.systems[last].staves[0]
        .bars
        .iter()
        .find(|b| b.bar == 20)
        .unwrap();
    let end = across.last().unwrap().1.x;
    assert!(
        (end - bar.beats[2].x).abs() < 0.01,
        "{end} vs {}",
        bar.beats[2].x
    );
    assert!((down[0].0.x - end).abs() < 0.01);
    // Every system it crosses has the drawings' lane: its staff is lower.
    for k in first..=last {
        assert!(
            e.hits.systems[k].staves[0].top - e.hits.systems[k].top
                > plain.hits.systems[k].staves[0].top - plain.hits.systems[k].top + sp,
            "system {k}"
        );
    }
}
