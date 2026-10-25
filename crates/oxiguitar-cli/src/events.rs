// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! `oxiguitar-cli events FILE`: the score as it plays (M4's timeline): the
//! bars in playing order with their times, every note with its start, end,
//! velocity and effects, and the metronome's clicks, as the golden files
//! in `fixtures/events/` hold them.

use std::process::ExitCode;

pub(crate) fn run(args: &[String]) -> ExitCode {
    let [path] = args else {
        return crate::usage_error("events needs one file");
    };
    let data = match std::fs::read(path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("{path}: cannot open: {e}");
            return ExitCode::FAILURE;
        }
    };
    match crate::plugins::open(path, &data) {
        Ok(opened) => {
            let timeline = oxiguitar_play::Timeline::new(&opened.score);
            print!("{}", oxiguitar_play::text::timeline(&timeline));
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{path}: {e}");
            ExitCode::FAILURE
        }
    }
}
