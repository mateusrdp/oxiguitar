// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! `info` and `detect`: print what a file contains.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;
use std::process::ExitCode;

use oxiguitar_formats::{Imported, Severity};
use oxiguitar_model::{BeatKind, Score, Staff, Track};

pub(crate) fn detect(args: &[String]) -> ExitCode {
    if args.is_empty() {
        return crate::usage_error("detect needs at least one file");
    }
    let mut ok = true;
    for path in args {
        match std::fs::read(path) {
            Ok(data) => match oxiguitar_formats::detect(&data) {
                Some(f) => println!("{path}: {f}{}", extension_note(path, f.usual_extension())),
                None => {
                    println!("{path}: unknown format");
                    ok = false;
                }
            },
            Err(e) => {
                eprintln!("{path}: cannot open: {e}");
                ok = false;
            }
        }
    }
    if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

pub(crate) fn run(args: &[String]) -> ExitCode {
    let bars = args.iter().any(|a| a == "--bars");
    let files: Vec<&String> = args.iter().filter(|a| !a.starts_with("--")).collect();
    if files.is_empty() {
        return crate::usage_error("info needs at least one file");
    }
    let mut ok = true;
    for (i, path) in files.iter().enumerate() {
        if i > 0 {
            println!();
        }
        let data = match std::fs::read(path) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("{path}: cannot open: {e}");
                ok = false;
                continue;
            }
        };
        match oxiguitar_formats::read(&data) {
            Ok(imported) => print!("{}", describe(path, &imported, bars)),
            Err(e) => {
                eprintln!("{path}: {e}");
                ok = false;
            }
        }
    }
    if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// A marker as `"[A] Verse"`, or `"Verse"` when it has no letter.
fn marker_label(m: &oxiguitar_model::Marker) -> String {
    if m.letter.is_empty() {
        format!("\"{}\"", m.text)
    } else {
        format!("[{}] \"{}\"", m.letter, m.text)
    }
}

fn extension_note(path: &str, usual: &str) -> String {
    let ext = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase);
    match ext {
        Some(e) if e == usual || (usual == "musicxml" && e == "xml") => String::new(),
        Some(e) => format!(" (named .{e})"),
        None => String::new(),
    }
}

/// The full text `info` prints for one file.
pub(crate) fn describe(path: &str, imported: &Imported, with_bars: bool) -> String {
    let s = &imported.score;
    let mut out = String::new();
    let _ = writeln!(out, "File:        {path}");
    let _ = writeln!(
        out,
        "Format:      {}{}",
        imported.format,
        extension_note(path, imported.format.usual_extension())
    );
    let info = &s.info;
    for (label, value) in [
        ("Title", &info.title),
        ("Subtitle", &info.subtitle),
        ("Artist", &info.artist),
        ("Album", &info.album),
        ("Words", &info.words),
        ("Music", &info.music),
        ("Transcriber", &info.transcriber),
        ("Copyright", &info.copyright),
    ] {
        if !value.trim().is_empty() {
            let _ = writeln!(out, "{:<12} {}", format!("{label}:"), value.trim());
        }
    }
    let _ = writeln!(
        out,
        "Tempo:       {} bpm{}",
        s.initial_tempo,
        if s.tempo_label.trim().is_empty() {
            String::new()
        } else {
            format!(" ({})", s.tempo_label.trim())
        }
    );
    let _ = writeln!(out, "Bars:        {}", s.bar_count());
    let _ = writeln!(out, "Time sigs:   {}", time_signatures(s));
    let repeats = s
        .master_bars
        .iter()
        .filter(|m| m.repeat_end.is_some())
        .count();
    let endings = s
        .master_bars
        .iter()
        .filter(|m| m.alternate_endings != 0)
        .count();
    let tempo_changes: usize = s.master_bars.iter().map(|m| m.tempo_changes.len()).sum();
    let _ = writeln!(
        out,
        "Structure:   {repeats} repeat(s), {endings} bar(s) in alternate endings, {tempo_changes} tempo change(s)"
    );
    let markers: Vec<String> = s
        .master_bars
        .iter()
        .enumerate()
        .filter_map(|(i, m)| {
            m.marker
                .as_ref()
                .map(|mk| format!("{} {}", i + 1, marker_label(mk)))
        })
        .collect();
    if !markers.is_empty() {
        let _ = writeln!(out, "Markers:     {}", markers.join(", "));
    }
    let _ = writeln!(out, "Tracks:      {}", s.tracks.len());
    for (i, t) in s.tracks.iter().enumerate() {
        let _ = writeln!(out, "  {:>2}. {}", i + 1, track_line(t));
    }
    let (short, long) = bar_length_problems(s);
    if short.is_empty() && long.is_empty() {
        let _ = writeln!(out, "Bar check:   every bar matches its time signature");
    } else {
        let _ = writeln!(
            out,
            "Bar check:   {} voice-bar(s) too short, {} too long (first: {})",
            short.len(),
            long.len(),
            short
                .iter()
                .chain(long.iter())
                .take(5)
                .cloned()
                .collect::<Vec<_>>()
                .join("; ")
        );
    }
    if imported.trailing_bytes > 0 {
        let _ = writeln!(
            out,
            "Trailing:    {} unread byte(s)",
            imported.trailing_bytes
        );
    }
    if imported.log.is_empty() {
        let _ = writeln!(out, "Import log:  empty");
    } else {
        let _ = writeln!(out, "Import log:");
        for e in imported.log.entries() {
            let sev = match e.severity {
                Severity::Info => "note",
                Severity::Warning => "warning",
            };
            let times = if e.count > 1 {
                format!(" ×{}", e.count)
            } else {
                String::new()
            };
            let _ = writeln!(
                out,
                "  {sev}: {}{times} (first: {})",
                e.message, e.first_place
            );
        }
    }
    if with_bars {
        out.push_str(&bar_table(s));
    }
    out
}

fn time_signatures(s: &Score) -> String {
    let mut counts: BTreeMap<(u8, u8), usize> = BTreeMap::new();
    for m in &s.master_bars {
        *counts
            .entry((m.time_signature.numerator, m.time_signature.denominator))
            .or_default() += 1;
    }
    let mut v: Vec<_> = counts.into_iter().collect();
    v.sort_by_key(|e| std::cmp::Reverse(e.1));
    v.iter()
        .map(|((n, d), c)| format!("{n}/{d} ×{c}"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn tuning_text(staff: &Staff) -> String {
    staff
        .tuning
        .iter()
        .rev()
        .map(|p| p.name())
        .collect::<Vec<_>>()
        .join(" ")
}

fn track_line(t: &Track) -> String {
    let mut parts = vec![if t.name.trim().is_empty() {
        "(no name)".to_owned()
    } else {
        t.name.trim().to_owned()
    }];
    if let Some(staff) = t.staves.first() {
        if t.instrument.percussion {
            parts.push("drums".into());
        } else {
            parts.push(format!(
                "{} strings {}",
                staff.string_count(),
                tuning_text(staff)
            ));
        }
        if staff.capo > 0 {
            parts.push(format!("capo {}", staff.capo));
        }
    }
    parts.push(format!("program {}", t.instrument.midi_program));
    let voices = t
        .staves
        .iter()
        .flat_map(|s| &s.bars)
        .map(|b| b.voices.len())
        .max()
        .unwrap_or(0);
    let notes: usize = t
        .staves
        .iter()
        .flat_map(|s| &s.bars)
        .flat_map(|b| &b.voices)
        .flat_map(|v| &v.beats)
        .map(|b| b.notes.len())
        .sum();
    parts.push(format!("{voices} voice(s)"));
    parts.push(format!("{notes} notes"));
    if !t.chords.is_empty() {
        parts.push(format!("{} chord diagram(s)", t.chords.len()));
    }
    if !t.lyrics.is_empty() {
        parts.push("lyrics".into());
    }
    parts.join(", ")
}

/// Voices whose length differs from their bar's time signature (EDIT-9).
/// Empty voices are skipped: Guitar Pro leaves unused voices empty.
fn bar_length_problems(s: &Score) -> (Vec<String>, Vec<String>) {
    let mut short = Vec::new();
    let mut long = Vec::new();
    for (ti, t) in s.tracks.iter().enumerate() {
        for staff in &t.staves {
            for (bi, (bar, mb)) in staff.bars.iter().zip(&s.master_bars).enumerate() {
                let Some(expected) = mb.expected_length() else {
                    continue;
                };
                for (vi, v) in bar.voices.iter().enumerate() {
                    if v.is_empty() {
                        continue;
                    }
                    let Some(len) = v.length() else { continue };
                    let place = format!("track {} bar {} voice {}", ti + 1, bi + 1, vi + 1);
                    if len < expected {
                        short.push(place);
                    } else if len > expected {
                        long.push(place);
                    }
                }
            }
        }
    }
    (short, long)
}

fn bar_table(s: &Score) -> String {
    let mut out =
        String::from("\nBars (track: beats/notes, then the bar's length if it is off):\n");
    for (bi, mb) in s.master_bars.iter().enumerate() {
        let ts = mb.time_signature;
        let mut line = format!("  {:>4} {:>2}/{:<2}", bi + 1, ts.numerator, ts.denominator);
        if mb.repeat_start {
            line.push_str(" |:");
        }
        if let Some(n) = mb.repeat_end {
            let _ = write!(line, " :|x{n}");
        }
        if mb.alternate_endings != 0 {
            let _ = write!(line, " [{:b}]", mb.alternate_endings);
        }
        if let Some(m) = &mb.marker {
            let _ = write!(line, " {}", marker_label(m));
        }
        for tc in &mb.tempo_changes {
            let _ = write!(line, " ♩={}@{}", tc.bpm, tc.position);
        }
        for (ti, t) in s.tracks.iter().enumerate() {
            let Some(bar) = t.staves.first().and_then(|st| st.bars.get(bi)) else {
                continue;
            };
            let beats: usize = bar.voices.iter().map(|v| v.beats.len()).sum();
            let notes: usize = bar
                .voices
                .iter()
                .flat_map(|v| &v.beats)
                .filter(|b| b.kind == BeatKind::Notes)
                .map(|b| b.notes.len())
                .sum();
            let _ = write!(line, " | t{}: {beats}/{notes}", ti + 1);
            if let (Some(expected), Some(first)) = (mb.expected_length(), bar.voices.first())
                && let Some(len) = first
                    .length()
                    .filter(|l| *l != expected && !first.is_empty())
            {
                let _ = write!(line, " len {len}");
            }
        }
        out.push_str(&line);
        out.push('\n');
    }
    out
}
