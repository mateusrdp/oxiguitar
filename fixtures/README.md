# Public test fixtures

Small files that are safe to redistribute: made by us for OxiGuitar, or carrying a license that allows it. CI runs on every one of them. Each file is listed here with its origin and license.

Files of unclear ownership never go here; see `corpus/README.md`.

| File | Format | Origin | License |
| --- | --- | --- | --- |
| `gp/oxiguitar-test-song.gp3` | Guitar Pro 3.00 | Written byte by byte by `crates/oxiguitar-formats/tests/support/gp_writer.rs` | GPL-3.0-or-later |
| `gp/oxiguitar-test-song.gp4` | Guitar Pro 4.06 | Same writer | GPL-3.0-or-later |
| `gp/oxiguitar-test-song.gp5` | Guitar Pro 5.10 | Same writer | GPL-3.0-or-later |

The three test songs hold the same music: a repeat, a marker, a time-signature change, a triplet, a hammer-on, a bend, a tie, a dead note, a rest, a chord and (in GP5) a second voice. To regenerate them after changing the writer: `OXIGUITAR_BLESS=1 cargo test -p oxiguitar-formats --test guitar_pro`.
