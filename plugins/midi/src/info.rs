// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! What a set's folder says about it: `oxiguitar-set.json`, written by the
//! sound-set installer beside each set it installs (#146, #165) and free
//! for anyone to write for a set of their own.
//!
//! ```json
//! {
//!   "name": "Black and Green Guitars (Karoryfer)",
//!   "role": "electric-guitars",
//!   "licence": "CC0",
//!   "credit": "Karoryfer Samples",
//!   "source": "https://github.com/sfzinstruments/karoryfer.black-and-green-guitars"
//! }
//! ```
//!
//! Every key is optional and unknown keys are left alone. The file sits in
//! the set's folder, at any depth below the `soundfonts` folder (so every
//! instrument of an SFZ set, in its `Programs` folder, finds its set's);
//! the nearest one wins. The `soundfonts` folder itself is never a set.
//! Nothing here is fetched: the file is read, and the `source` shown as
//! text, never opened.

use std::path::Path;

use oxiguitar_plugin::serde_json::{self, Value};

use crate::settings::Role;

/// The file's name.
pub(crate) const FILE: &str = "oxiguitar-set.json";

/// The most such a file may hold; a bigger one is not read.
const MAX_BYTES: u64 = 64 * 1024;

/// What the file says.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Info {
    /// What to call the set.
    pub(crate) name: Option<String>,
    /// The kind of track it is for, which the settings window suggests it
    /// for.
    pub(crate) role: Option<Role>,
    /// Its licence, as its authors name it ("CC0", "CC BY 4.0").
    pub(crate) licence: Option<String>,
    /// Who made it, to be credited.
    pub(crate) credit: Option<String>,
    /// Where it comes from, as text.
    pub(crate) source: Option<String>,
}

impl Info {
    /// Reads the file's text. `None` when it isn't a JSON object or says
    /// nothing usable.
    pub(crate) fn parse(text: &str) -> Option<Info> {
        let Value::Object(map) = serde_json::from_str::<Value>(text).ok()? else {
            return None;
        };
        let string = |key: &str| {
            map.get(key)
                .and_then(Value::as_str)
                .map(|s| s.trim().chars().take(200).collect::<String>())
                .filter(|s| !s.is_empty())
        };
        let info = Info {
            name: string("name"),
            role: string("role").and_then(|r| Role::from_key(&r)),
            // Both spellings, as the installer's author may use either.
            licence: string("licence").or_else(|| string("license")),
            credit: string("credit"),
            source: string("source"),
        };
        (info != Info::default()).then_some(info)
    }
}

/// The info of the set at `set`, a file found under `root` (the folder the
/// search began in): the nearest `oxiguitar-set.json` in the folders from
/// the file's own up to, but not including, `root`.
pub(crate) fn of(set: &Path, root: &Path) -> Option<Info> {
    let mut dir = set.parent()?;
    while dir != root && dir.starts_with(root) {
        let file = dir.join(FILE);
        if let Ok(meta) = std::fs::metadata(&file)
            && meta.is_file()
            && meta.len() <= MAX_BYTES
            && let Ok(text) = std::fs::read_to_string(&file)
        {
            // A file that can't be read as info ends the search: it is the
            // nearest, and the folder above it is another set's.
            return Info::parse(&text);
        }
        dir = dir.parent()?;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_file_is_read_leniently() {
        let info = Info::parse(
            r#"{"name": " Black and Green Guitars (Karoryfer) ", "role": "electric-guitars",
                "license": "CC0", "credit": "Karoryfer Samples", "extra": [1, 2]}"#,
        )
        .unwrap();
        assert_eq!(
            info.name.as_deref(),
            Some("Black and Green Guitars (Karoryfer)")
        );
        assert_eq!(info.role, Some(Role::ElectricGuitars));
        assert_eq!(info.licence.as_deref(), Some("CC0"));
        assert_eq!(info.credit.as_deref(), Some("Karoryfer Samples"));
        assert_eq!(info.source, None);
        // The British spelling wins when both are there.
        let both = Info::parse(r#"{"licence": "CC BY 4.0", "license": "CC0"}"#).unwrap();
        assert_eq!(both.licence.as_deref(), Some("CC BY 4.0"));
        // An unknown role, a wrong type, or nothing: ignored.
        let odd = Info::parse(r#"{"name": "X", "role": "tubas", "credit": 5}"#).unwrap();
        assert_eq!((odd.role, odd.credit), (None, None));
        for text in ["", "[]", "{", "{}", r#"{"name": "  "}"#, r#"{"name": 5}"#] {
            assert_eq!(Info::parse(text), None, "{text:?}");
        }
    }

    #[test]
    fn the_nearest_file_below_the_root_wins() {
        let dir = std::env::temp_dir().join(format!("oxiguitar-midi-info-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let root = dir.join("soundfonts");
        let set = root.join("Black and Green Guitars (Karoryfer)");
        std::fs::create_dir_all(set.join("Programs/inner")).unwrap();
        std::fs::write(set.join(FILE), r#"{"name": "Whole set", "role": "basses"}"#).unwrap();
        std::fs::write(
            set.join("Programs/inner").join(FILE),
            r#"{"name": "Inner part"}"#,
        )
        .unwrap();
        // The root's own file labels no set.
        std::fs::write(root.join(FILE), r#"{"name": "Everything"}"#).unwrap();
        std::fs::write(root.join("loose.sf2"), b"").unwrap();

        let name = |p: &Path| of(p, &root).and_then(|i| i.name);
        assert_eq!(
            name(&set.join("Programs/01.sfz")).as_deref(),
            Some("Whole set")
        );
        assert_eq!(
            name(&set.join("Programs/inner/02.sfz")).as_deref(),
            Some("Inner part")
        );
        assert_eq!(name(&root.join("loose.sf2")), None);
        // A file that says nothing usable still ends the search there.
        std::fs::write(set.join("Programs/inner").join(FILE), "oops").unwrap();
        assert_eq!(name(&set.join("Programs/inner/02.sfz")), None);
        // Outside the root nothing is looked at.
        assert_eq!(of(&dir.join("elsewhere/x.sf2"), &root), None);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
