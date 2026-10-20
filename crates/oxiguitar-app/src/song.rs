// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The Song tab's fields (M3 slice 6), without window code: the score's
//! information in Guitar Pro 8's order (its Song Inspector, "Song
//! Information", checked 20 Oct 2026; Mateus, 20 Oct 2026: a Song tab
//! beside the Inspector).

use oxiguitar_model::ScoreInfo;

/// One field.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Field {
    pub(crate) id: &'static str,
    pub(crate) label: &'static str,
    pub(crate) text: String,
    /// Several lines (notices, instructions).
    pub(crate) multiline: bool,
}

/// The fields, in Guitar Pro 8's order.
pub(crate) fn fields(info: &ScoreInfo) -> Vec<Field> {
    let one = |id, label, text: &str| Field {
        id,
        label,
        text: text.to_owned(),
        multiline: false,
    };
    vec![
        one("title", "Title", &info.title),
        one("artist", "Artist", &info.artist),
        one("subtitle", "Subtitle", &info.subtitle),
        one("album", "Album", &info.album),
        one("words", "Words", &info.words),
        one("music", "Music", &info.music),
        one("copyright", "Copyright", &info.copyright),
        one("transcriber", "Transcriber", &info.transcriber),
        Field {
            id: "notices",
            label: "Notices",
            text: info.notices.join("\n"),
            multiline: true,
        },
        Field {
            id: "instructions",
            label: "Instructions",
            text: info.instructions.clone(),
            multiline: true,
        },
    ]
}

/// Field `id` set to `text`: whether it changed.
pub(crate) fn set(info: &mut ScoreInfo, id: &str, text: &str) -> bool {
    let before = info.clone();
    let slot = match id {
        "title" => &mut info.title,
        "artist" => &mut info.artist,
        "subtitle" => &mut info.subtitle,
        "album" => &mut info.album,
        "words" => &mut info.words,
        "music" => &mut info.music,
        "copyright" => &mut info.copyright,
        "transcriber" => &mut info.transcriber,
        "instructions" => &mut info.instructions,
        "notices" => {
            info.notices = text.lines().map(str::to_owned).collect();
            *info = oxiguitar_edit::tidy_info(info);
            return *info != before;
        }
        _ => return false,
    };
    text.clone_into(slot);
    *info = oxiguitar_edit::tidy_info(info);
    *info != before
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fields_in_guitar_pro_s_order_and_set() {
        let mut info = ScoreInfo {
            title: "Song".into(),
            notices: vec!["a".into(), "b".into()],
            ..ScoreInfo::default()
        };
        let f = fields(&info);
        let labels: Vec<&str> = f.iter().map(|f| f.label).collect();
        assert_eq!(
            labels,
            [
                "Title",
                "Artist",
                "Subtitle",
                "Album",
                "Words",
                "Music",
                "Copyright",
                "Transcriber",
                "Notices",
                "Instructions"
            ]
        );
        assert_eq!(f[8].text, "a\nb");
        assert!(!set(&mut info, "title", " Song "), "the same, trimmed");
        assert!(set(&mut info, "artist", "Band"));
        assert_eq!(info.artist, "Band");
        assert!(set(&mut info, "notices", "x\n\ny\n\n"));
        assert_eq!(info.notices, ["x", "", "y"]);
        assert!(!set(&mut info, "nonsense", "z"));
    }
}
