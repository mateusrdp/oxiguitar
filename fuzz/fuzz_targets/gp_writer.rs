// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The .gp writer: any file a reader takes, written as .gp. What we write
//! must read back, and writing that again must lose nothing and read back
//! the same (FMT-4). Seeds: every public fixture.

#![no_main]

use libfuzzer_sys::fuzz_target;
use oxiguitar_formats::{read, write_gp};

fuzz_target!(|data: &[u8]| {
    let Ok(first) = read(data) else {
        return;
    };
    let Ok(written) = write_gp(&first.score) else {
        return;
    };
    let second = read(&written.data).expect("what the writer wrote reads back");
    let again = write_gp(&second.score).expect("a score read back is written again");
    assert_eq!(
        again.losses.warning_count(),
        0,
        "a second round trip loses {:?}",
        again.losses.entries()
    );
    let third = read(&again.data).expect("the second file reads back");
    assert!(
        third.score == second.score,
        "the second round trip changes the score"
    );
});
