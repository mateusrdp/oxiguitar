// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! What OxiGuitar knows about the recommended SFZ sets, beyond what their
//! files say (docs/design/player.md, "SFZ, as built"): which key switch or
//! controller plays each articulation the score asks for, and where a
//! drum kit that isn't laid out as General MIDI keeps each drum.
//!
//! A set is recognised by its instrument file's name and its folders, as
//! its authors ship it. Any other SFZ instrument plays without a map: its
//! notes as they are, palm mutes and dead notes ended early as on the
//! SoundFont path.

use std::path::Path;

/// An articulation: the key switch to press and the controllers to set
/// before the note.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Articulation {
    pub(crate) switch: Option<u8>,
    pub(crate) controllers: &'static [(u16, f32)],
}

/// A General MIDI drum's key, the kit's key for it, and a controller to
/// set first (a hi-hat's openness).
pub(crate) type Drum = (u8, u8, Option<(u16, f32)>);

/// What a set plays for each articulation, and where its drums are.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Map {
    /// The set's name, for messages and tests.
    pub(crate) name: &'static str,
    /// Where the kit keeps General MIDI's drums.
    pub(crate) keys: &'static [Drum],
    /// Ordinary notes: what every other articulation goes back from.
    pub(crate) normal: Option<Articulation>,
    pub(crate) palm_mute: Option<Articulation>,
    pub(crate) dead: Option<Articulation>,
    pub(crate) ghost: Option<Articulation>,
    /// Recorded hammer-ons and pull-offs, for notes not picked: they sound
    /// more natural than carrying on the ringing string (Mateus, 26 Oct
    /// 2026), which sets without them still do.
    pub(crate) legato: Option<Articulation>,
}

const NONE: Map = Map {
    name: "",
    keys: &[],
    normal: None,
    palm_mute: None,
    dead: None,
    ghost: None,
    legato: None,
};

/// Karoryfer's Black and Green Guitars, its key-switched programs: Twang
/// on 36, Staccato on 37, Hammer-on on 38; controller 70 mutes (its
/// envelopes shorten), the palm on the strings.
const BLACK_AND_GREEN: Map = Map {
    name: "Black and Green Guitars",
    normal: Some(Articulation {
        switch: Some(36),
        controllers: &[(70, 0.0)],
    }),
    palm_mute: Some(Articulation {
        switch: Some(36),
        controllers: &[(70, 127.0)],
    }),
    dead: Some(Articulation {
        switch: Some(37),
        controllers: &[(70, 0.0)],
    }),
    legato: Some(Articulation {
        switch: Some(38),
        controllers: &[(70, 0.0)],
    }),
    ..NONE
};

/// Its combined program, without hammer-ons.
const BLACK_AND_GREEN_COMBO: Map = Map {
    name: "Black and Green Guitars",
    legato: None,
    ..BLACK_AND_GREEN
};

/// Karoryfer's Black and Blue Basses, its key-switched programs: Pluck on
/// 31, Ghost on 30, Staccato on 29.
const BLACK_AND_BLUE: Map = Map {
    name: "Black and Blue Basses",
    normal: Some(Articulation {
        switch: Some(31),
        controllers: &[],
    }),
    ghost: Some(Articulation {
        switch: Some(30),
        controllers: &[],
    }),
    dead: Some(Articulation {
        switch: Some(29),
        controllers: &[],
    }),
    palm_mute: Some(Articulation {
        switch: Some(29),
        controllers: &[],
    }),
    ..NONE
};

/// SM Drums' whole kit: kick 36, snares 38 and 40 (and 41), side stick
/// 37, rim 39; hi-hat closed 42, open 46, pedal 50; toms 43 (floor), 45,
/// 47, 48; crashes 54 to 57, china 58; rides 60 to 63 (17" and 20", bow
/// and bell).
const SM_DRUMS: Map = Map {
    name: "SM Drums",
    keys: &[
        (35, 36, None),
        (41, 43, None),
        (44, 50, None),
        (49, 56, None),
        (50, 48, None),
        (51, 62, None),
        (52, 58, None),
        (53, 63, None),
        (55, 54, None),
        (57, 57, None),
        (59, 60, None),
    ],
    ..NONE
};

/// Naked Drums' own layout (its programs but the General MIDI one): toms
/// 41 to 48 as General MIDI's, hi-hat open on 49, crashes on 55 and 57,
/// splashes 54 and 56, chinas 50 and 52.
const NAKED_DRUMS: Map = Map {
    name: "Naked Drums",
    keys: &[
        (35, 36, None),
        (46, 49, None),
        (49, 55, None),
        (50, 48, None),
        (52, 50, None),
        (55, 54, None),
        (59, 51, None),
    ],
    ..NONE
};

/// Karoryfer's kits (Big Rusty, Unruly, Swirly) are laid out as General
/// MIDI, but their hi-hat on 46 opens as controller 4 rises.
const KARORYFER_KIT: Map = Map {
    name: "Karoryfer drums",
    keys: &[(35, 36, None), (46, 46, Some((4, 127.0)))],
    ..NONE
};

/// The map for the instrument at `path`, if it is one of the recommended
/// sets that needs one.
pub(crate) fn for_instrument(path: &Path) -> Option<&'static Map> {
    let name = path.file_name()?.to_str()?;
    let folders: Vec<String> = path
        .ancestors()
        .skip(1)
        .filter_map(|a| a.file_name())
        .map(|n| {
            n.to_string_lossy()
                .to_lowercase()
                .replace(['-', '_', '.'], " ")
        })
        .collect();
    let under = |words: &str| folders.iter().any(|f| f.contains(words));
    Some(match name {
        "01-green_keyswitch.sfz" | "02-black_keyswitch.sfz" => &BLACK_AND_GREEN,
        "03-combo_keyswitch.sfz" => &BLACK_AND_GREEN_COMBO,
        "01-darkblack_keysw.sfz" | "02-darkblack_keysw_warm.sfz" => &BLACK_AND_BLUE,
        "SM_Drums_kit.sfz" => &SM_DRUMS,
        n if under("naked drums") && !n.contains("GM") => &NAKED_DRUMS,
        _ if under("swirly drums") || under("big rusty drums") || under("unruly drums") => {
            &KARORYFER_KIT
        }
        _ => return None,
    })
}

impl Map {
    /// The kit's key for General MIDI drum `key`, and a controller to set
    /// first.
    pub(crate) fn key(&self, key: u8) -> (u8, Option<(u16, f32)>) {
        self.keys
            .iter()
            .find(|(gm, _, _)| *gm == key)
            .map_or((key, None), |&(_, kit, cc)| (kit, cc))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_recommended_sets_are_recognised_by_their_files() {
        let name = |p: &str| for_instrument(Path::new(p)).map(|m| m.name);
        assert_eq!(
            name("/u/soundfonts/karoryfer.black-and-green-guitars/Programs/01-green_keyswitch.sfz"),
            Some("Black and Green Guitars")
        );
        assert!(
            for_instrument(Path::new("/x/Programs/03-combo_keyswitch.sfz"))
                .unwrap()
                .legato
                .is_none()
        );
        assert_eq!(
            name("/u/soundfonts/Black And Blue Basses/Programs/01-darkblack_keysw.sfz"),
            Some("Black and Blue Basses")
        );
        assert_eq!(
            name("/u/soundfonts/SMDrums/Programs/SM_Drums_kit.sfz"),
            Some("SM Drums")
        );
        assert_eq!(
            name("/u/soundfonts/Naked/Wilkinson Audio/Naked Drums/Stereo/Naked Drums.sfz"),
            Some("Naked Drums")
        );
        assert_eq!(
            name("/u/soundfonts/Naked/Wilkinson Audio/Naked Drums/User/Naked Drums GM.sfz"),
            None
        );
        assert_eq!(
            name("/u/soundfonts/karoryfer.swirly-drums/Programs/Full_kit.sfz"),
            Some("Karoryfer drums")
        );
        assert_eq!(
            name("/u/soundfonts/Big Rusty Drums/Programs/01-full.sfz"),
            Some("Karoryfer drums")
        );
        assert_eq!(name("/u/soundfonts/FreePats/clean.sfz"), None);
    }

    #[test]
    fn drums_move_to_the_kits_keys() {
        assert_eq!(SM_DRUMS.key(49), (56, None));
        assert_eq!(SM_DRUMS.key(38), (38, None));
        assert_eq!(NAKED_DRUMS.key(46), (49, None));
        assert_eq!(KARORYFER_KIT.key(46), (46, Some((4, 127.0))));
    }
}
