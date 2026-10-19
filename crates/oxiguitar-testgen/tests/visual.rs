// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Visual regression (M1): the fifty reference scores engrave to the
//! pictures stored beside them in `fixtures/reference/`.
//!
//! Each reference is a Guitar Pro 7 file made by `oxiguitar-testgen`,
//! read back as the app reads it, through the Guitar Pro 7–8 format plugin
//! built as the `gp.oxp` file we ship, engraved with its options and drawn
//! at 1.5 pixels per point (108 dpi). The pictures are compared with a
//! tolerance, not byte for byte, because anti-aliasing and floating point
//! may differ a little between machines: a pixel counts as changed when
//! one of its channels moved by more than [`PIXEL_TOLERANCE`], and a
//! picture fails when more than [`MAX_CHANGED`] of its pixels changed, or
//! when its size changed. A dot that goes missing fails it; so does a
//! notehead moved by a fraction of a point.
//!
//! On failure the test writes what it drew and a picture of the
//! differences (changes in red) to `target/tmp/visual-diff/`; CI keeps
//! that folder as an artifact of the run.
//!
//! After an intended change to engraving, or to the references themselves,
//! run
//!
//! ```sh
//! OXIGUITAR_BLESS=1 cargo test -p oxiguitar-testgen --test visual
//! ```
//!
//! and look at every picture that changed before committing them (`git
//! status fixtures/reference`).

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use oxiguitar_display::smufl::Metadata;
use oxiguitar_display::{FontSet, Theme};
use oxiguitar_host::api::message::Level;
use oxiguitar_host::{Plugins, SharedSettings, Unheard};
use oxiguitar_layout::{Layout, engrave};
use oxiguitar_render::{Raster, View};
use oxiguitar_testgen::{Reference, references, to_gp, to_gpif};

/// Pixels per point: 108 dpi, enough to see every symbol.
const SCALE: f32 = 1.5;
/// A pixel counts as changed when a channel moved by more than this.
const PIXEL_TOLERANCE: u8 = 40;
/// A picture fails when more than this many of its pixels changed.
const MAX_CHANGED: usize = 4;

fn reference_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/reference")
}

/// Our format plugins, built once (a system loads a library file once per
/// process, so every reference goes through this one host).
fn plugins() -> &'static Plugins {
    static PLUGINS: OnceLock<Plugins> = OnceLock::new();
    PLUGINS.get_or_init(|| {
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let folder = oxiguitar_plugin_test::build_ours(&workspace, !cfg!(debug_assertions))
            .expect("our plugins build");
        Plugins::discover(
            &[folder],
            SharedSettings::default(),
            oxiguitar_host::display_system(),
            Arc::new(Unheard),
        )
    })
}

fn diff_dir() -> PathBuf {
    Path::new(env!("CARGO_TARGET_TMPDIR")).join("visual-diff")
}

fn blessing() -> bool {
    std::env::var_os("OXIGUITAR_BLESS").is_some_and(|v| !v.is_empty())
}

/// An opaque picture, RGB.
struct Picture {
    width: u32,
    height: u32,
    rgb: Vec<u8>,
}

/// The picture files of a reference: `name-1.png`, `name-2.png`, …
fn picture_name(r: &Reference, page: usize) -> String {
    format!("{}-{}.png", r.name, page + 1)
}

/// The score.gpif inside a stored .gp file.
fn stored_gpif(path: &Path) -> Option<String> {
    let data = std::fs::read(path).ok()?;
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(data)).ok()?;
    let mut entry = zip.by_name("Content/score.gpif").ok()?;
    let mut text = String::new();
    entry.read_to_string(&mut text).ok()?;
    Some(text)
}

/// Engraves and draws a reference from its stored file.
fn draw(
    r: &Reference,
    file: &Path,
    fonts: &FontSet,
    meta: &Metadata,
) -> Result<Vec<Picture>, String> {
    let data = std::fs::read(file).map_err(|e| format!("cannot read {}: {e}", file.display()))?;
    let plugins = plugins();
    let format = plugins
        .recognise(&data, Some("gp"))
        .map_err(|e| format!("does not open: {e}"))?;
    let read = plugins
        .read(format, &data)
        .map_err(|e| format!("does not open: {e}"))?;
    let warnings: Vec<String> = read
        .log
        .iter()
        .filter(|e| e.level >= Level::Warning)
        .map(|e| format!("{} ({})", e.message, e.place))
        .collect();
    if !warnings.is_empty() {
        return Err(format!("the reader warns: {}", warnings.join("; ")));
    }
    let score = oxiguitar_model::wire::from_wire2(read.score)?;
    let engraved = engrave(&score, fonts, meta, &r.options);
    let theme = if r.dark { Theme::DARK } else { Theme::LIGHT };
    let mut raster = Raster::new();
    let mut out = Vec::new();
    for page in &engraved.pages {
        let view = View::page(page, SCALE);
        let pixmap = raster
            .render(page, fonts, &theme, &view)
            .ok_or("an empty sheet")?;
        // The page is opaque, so premultiplied and plain colours agree.
        let rgb = pixmap
            .data()
            .as_chunks::<4>()
            .0
            .iter()
            .flat_map(|p| [p[0], p[1], p[2]])
            .collect();
        out.push(Picture {
            width: pixmap.width(),
            height: pixmap.height(),
            rgb,
        });
    }
    Ok(out)
}

fn write_png(path: &Path, pic: &Picture) {
    let file = std::fs::File::create(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), pic.width, pic.height);
    enc.set_depth(png::BitDepth::Eight);
    enc.set_compression(png::Compression::High);
    // Grey pictures (most of them) take a third of the room.
    let grey = pic
        .rgb
        .as_chunks::<3>()
        .0
        .iter()
        .all(|p| p[0] == p[1] && p[1] == p[2]);
    let data: Vec<u8> = if grey {
        enc.set_color(png::ColorType::Grayscale);
        pic.rgb.as_chunks::<3>().0.iter().map(|p| p[0]).collect()
    } else {
        enc.set_color(png::ColorType::Rgb);
        pic.rgb.clone()
    };
    let mut w = enc.write_header().expect("PNG header");
    w.write_image_data(&data).expect("PNG data");
}

fn read_png(path: &Path) -> Result<Picture, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut dec = png::Decoder::new(std::io::BufReader::new(file));
    dec.set_transformations(png::Transformations::EXPAND);
    let mut reader = dec
        .read_info()
        .map_err(|e| format!("{}: {e}", path.display()))?;
    let mut buf = vec![0; reader.output_buffer_size().ok_or("PNG too large")?];
    let info = reader
        .next_frame(&mut buf)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    let bytes = &buf[..info.buffer_size()];
    let rgb = match info.color_type {
        png::ColorType::Grayscale => bytes.iter().flat_map(|g| [*g, *g, *g]).collect(),
        png::ColorType::GrayscaleAlpha => bytes
            .as_chunks::<2>()
            .0
            .iter()
            .flat_map(|p| [p[0], p[0], p[0]])
            .collect(),
        png::ColorType::Rgb => bytes.to_vec(),
        png::ColorType::Rgba => bytes
            .as_chunks::<4>()
            .0
            .iter()
            .flat_map(|p| [p[0], p[1], p[2]])
            .collect(),
        other => {
            return Err(format!(
                "{}: unexpected colour type {other:?}",
                path.display()
            ));
        }
    };
    Ok(Picture {
        width: info.width,
        height: info.height,
        rgb,
    })
}

/// How two pictures of the same size differ: changed pixels, the largest
/// change, and a picture of the changes.
fn compare(want: &Picture, got: &Picture) -> (usize, u8, Picture) {
    let mut changed = 0;
    let mut largest = 0;
    let mut rgb = Vec::with_capacity(got.rgb.len());
    for (w, g) in want
        .rgb
        .as_chunks::<3>()
        .0
        .iter()
        .zip(got.rgb.as_chunks::<3>().0.iter())
    {
        let d = w
            .iter()
            .zip(g)
            .map(|(a, b)| a.abs_diff(*b))
            .max()
            .unwrap_or(0);
        largest = largest.max(d);
        if d > PIXEL_TOLERANCE {
            changed += 1;
            rgb.extend_from_slice(&[230, 0, 0]);
        } else {
            // The new picture, faded, so the changes stand out.
            let grey = (u16::from(g[0]) + u16::from(g[1]) + u16::from(g[2])) / 3;
            let faded = (180 + grey * 75 / 255) as u8;
            rgb.extend_from_slice(&[faded, faded, faded]);
        }
    }
    (
        changed,
        largest,
        Picture {
            width: got.width,
            height: got.height,
            rgb,
        },
    )
}

/// The index of the references, `fixtures/reference/README.md`.
fn index(refs: &[Reference], pages: &[usize]) -> String {
    let mut s = String::from(
        "# Visual-regression references\n\n\
         Generated by `oxiguitar-testgen` (`crates/oxiguitar-testgen/src/references.rs`); do not edit by hand. \
         Each reference is a Guitar Pro 7 file and the pictures it engraves to at 1.5 pixels per point. \
         `cargo test -p oxiguitar-testgen --test visual` checks them; see `docs/testing.md`. \
         License: GPL-3.0-or-later, like the rest of OxiGuitar.\n\n\
         | Reference | Shows | Layout | Pictures |\n| --- | --- | --- | --- |\n",
    );
    for (r, n) in refs.iter().zip(pages) {
        let layout = match r.options.layout {
            Layout::Page => "Page",
            Layout::Vertical => "Vertical",
            Layout::Horizontal => "Horizontal",
            Layout::Parchment => "Parchment",
        };
        let theme = if r.dark { ", dark" } else { "" };
        let _ = writeln!(
            s,
            "| [`{name}.gp`]({name}.gp) | {} | {layout}{theme} | [{n}]({name}-1.png) |",
            r.about,
            name = r.name,
        );
    }
    s
}

/// What checking one reference found.
#[derive(Default)]
struct Outcome {
    pages: usize,
    /// The reference's files, which must be in the folder.
    files: Vec<String>,
    problems: Vec<String>,
    /// Files written while blessing.
    changed: Vec<String>,
}

fn check_one(r: &Reference, dir: &Path, bless: bool, fonts: &FontSet, meta: &Metadata) -> Outcome {
    let mut out = Outcome::default();
    let gp_path = dir.join(format!("{}.gp", r.name));
    out.files.push(format!("{}.gp", r.name));
    // The stored score is what the generator makes now.
    let gpif = to_gpif(&r.song);
    if stored_gpif(&gp_path).as_deref() != Some(gpif.as_str()) {
        if bless {
            std::fs::write(&gp_path, to_gp(&r.song)).expect("write reference score");
            out.changed.push(format!("{}.gp", r.name));
        } else {
            out.problems.push(format!(
                "{}: the stored score is not what the generator makes (missing, or the generator changed)",
                r.name
            ));
            return out;
        }
    }
    let pictures = match draw(r, &gp_path, fonts, meta) {
        Ok(p) => p,
        Err(e) => {
            out.problems.push(format!("{}: {e}", r.name));
            return out;
        }
    };
    out.pages = pictures.len();
    for (i, got) in pictures.iter().enumerate() {
        let name = picture_name(r, i);
        out.files.push(name.clone());
        let path = dir.join(&name);
        let want = read_png(&path);
        let verdict = match &want {
            Err(e) => Some(format!("no stored picture ({e})")),
            Ok(want) if (want.width, want.height) != (got.width, got.height) => Some(format!(
                "size changed from {}×{} to {}×{} pixels",
                want.width, want.height, got.width, got.height
            )),
            // Blessing keeps the stored picture exact: any change is
            // written, even one the tolerance lets through.
            Ok(want) if bless && want.rgb != got.rgb => Some("changed".to_owned()),
            Ok(want) => {
                let (changed, largest, diff) = compare(want, got);
                let total = (got.width * got.height) as usize;
                if changed > MAX_CHANGED {
                    if !bless {
                        let _ = std::fs::create_dir_all(diff_dir());
                        write_png(
                            &diff_dir().join(format!("{}-{}-diff.png", r.name, i + 1)),
                            &diff,
                        );
                    }
                    Some(format!(
                        "{changed} of {total} pixels changed (largest change {largest} of 255)"
                    ))
                } else {
                    None
                }
            }
        };
        if let Some(why) = verdict {
            if bless {
                write_png(&path, got);
                out.changed.push(name);
            } else {
                let _ = std::fs::create_dir_all(diff_dir());
                write_png(&diff_dir().join(&name), got);
                out.problems.push(format!("{name}: {why}"));
            }
        }
    }
    out
}

#[test]
fn references_engrave_as_stored() {
    let dir = reference_dir();
    let bless = blessing();
    if bless {
        std::fs::create_dir_all(&dir).expect("reference folder");
    }
    let _ = std::fs::remove_dir_all(diff_dir());
    let fonts = FontSet::bundled();
    let meta = Metadata::bravura().expect("Bravura metadata");
    let refs = references();
    let mut problems: Vec<String> = Vec::new();
    let mut expected: BTreeSet<String> = BTreeSet::from(["README.md".to_owned()]);
    let mut page_counts = Vec::new();
    let mut changed_files = Vec::new();

    // Several references at once: a debug build draws slowly.
    let threads = std::thread::available_parallelism().map_or(2, |n| n.get());
    let chunk = refs.len().div_ceil(threads);
    let outcomes: Vec<Outcome> = std::thread::scope(|scope| {
        let handles: Vec<_> = refs
            .chunks(chunk)
            .map(|part| {
                let (dir, fonts, meta) = (&dir, &fonts, &meta);
                scope.spawn(move || {
                    part.iter()
                        .map(|r| check_one(r, dir, bless, fonts, meta))
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|h| h.join().expect("a reference check panicked"))
            .collect()
    });
    for o in outcomes {
        page_counts.push(o.pages);
        expected.extend(o.files);
        problems.extend(o.problems);
        changed_files.extend(o.changed);
    }

    // The index, and no stale files.
    let readme = dir.join("README.md");
    let text = index(&refs, &page_counts);
    if std::fs::read_to_string(&readme).ok().as_deref() != Some(text.as_str()) {
        if bless {
            std::fs::write(&readme, &text).expect("write the index");
            changed_files.push("README.md".to_owned());
        } else {
            problems.push("README.md: the index is out of date".to_owned());
        }
    }
    let present: BTreeSet<String> = std::fs::read_dir(&dir)
        .map(|d| {
            d.filter_map(|e| e.ok())
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    for stale in present.difference(&expected) {
        if bless {
            let _ = std::fs::remove_file(dir.join(stale));
            changed_files.push(format!("{stale} (removed)"));
        } else {
            problems.push(format!("{stale}: not a reference any more"));
        }
    }

    if bless {
        println!(
            "blessed {} file(s): {}",
            changed_files.len(),
            changed_files.join(", ")
        );
    }
    assert!(
        problems.is_empty(),
        "{} problem(s); pictures in {}. After an intended change, run \
         OXIGUITAR_BLESS=1 cargo test -p oxiguitar-testgen --test visual\n{}",
        problems.len(),
        diff_dir().display(),
        problems.join("\n")
    );
}

#[test]
fn a_moved_symbol_is_caught() {
    // The tolerance lets anti-aliasing through, not a real change: shift
    // a reference's picture by one pixel, or take out a dot, and it fails.
    let fonts = FontSet::bundled();
    let meta = Metadata::bravura().expect("Bravura metadata");
    let r = &references()[0];
    let dir = std::env::temp_dir().join(format!("oxiguitar-visual-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temporary folder");
    let file = dir.join("r.gp");
    std::fs::write(&file, to_gp(&r.song)).expect("write");
    let pics = draw(r, &file, &fonts, &meta).expect("draws");
    let _ = std::fs::remove_dir_all(&dir);
    let a = &pics[0];
    let mut shifted = a.rgb.clone();
    let row = 3 * a.width as usize;
    // Everything one pixel to the right.
    for line in shifted.chunks_exact_mut(row) {
        line.copy_within(0..row - 3, 3);
    }
    let b = Picture {
        width: a.width,
        height: a.height,
        rgb: shifted,
    };
    let (same, ..) = compare(a, a);
    let (moved, ..) = compare(a, &b);
    assert_eq!(same, 0);
    assert!(moved > MAX_CHANGED, "{moved} pixels");
    // A staccato dot is about 3 pixels across at this scale: paint the
    // darkest 3 × 3 block of the picture white.
    let mut dotless = a.rgb.clone();
    let w = a.width as usize;
    let dark = (0..a.rgb.len() / 3)
        .find(|i| a.rgb[3 * i] < 40 && i % w + 3 < w)
        .expect("ink");
    for dy in 0..3 {
        for dx in 0..3 {
            let k = 3 * (dark + dy * w + dx);
            dotless[k..k + 3].copy_from_slice(&[255, 255, 255]);
        }
    }
    let c = Picture {
        width: a.width,
        height: a.height,
        rgb: dotless,
    };
    let (gone, ..) = compare(a, &c);
    assert!(gone > MAX_CHANGED, "{gone} pixels");
}
