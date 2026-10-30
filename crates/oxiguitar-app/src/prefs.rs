// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The View menu's settings that last from one session to the next, kept
//! under the `view` key of settings.json. They are things a user sets once
//! (Mateus, 18 Oct 2026), so the app remembers them. No window code here.

use oxiguitar_layout::{Layout, Paper};
use serde_json::{Map, Value, json};

/// The settings key.
pub(crate) const KEY: &str = "view";

/// Names in settings.json, in `Layout::ALL` and `Paper::ALL` order.
const LAYOUTS: [&str; 4] = ["page", "vertical", "horizontal", "parchment"];
const PAPERS: [&str; 2] = ["a4", "letter"];

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ViewPrefs {
    /// The window's own colours: dark by default (Mateus, 18 Oct 2026).
    pub(crate) dark_interface: bool,
    /// Light notation on a dark page; off by default, so the page is white.
    pub(crate) dark_page: bool,
    /// Dynamics on the page; off by default, as they clutter it (Mateus,
    /// 20 Oct 2026).
    pub(crate) dynamics: bool,
    pub(crate) layout: Layout,
    pub(crate) paper: Paper,
    /// Score zoom factor (0.25–4.0); persisted as a raw f32.
    pub(crate) score_zoom: f32,
    /// Interface zoom factor (snaps to UI_ZOOMS steps); persisted as a raw
    /// f32. Both default to 1.0 so the user's zoom is remembered across
    /// restarts (Mateus, 31 Oct 2026).
    pub(crate) ui_zoom: f32,
}

// f32 doesn't implement Eq (NaN != NaN), but our zoom values are never NaN.
impl Eq for ViewPrefs {}

impl Default for ViewPrefs {
    fn default() -> ViewPrefs {
        ViewPrefs {
            dark_interface: true,
            dark_page: false,
            dynamics: false,
            layout: Layout::Page,
            paper: Paper::A4,
            score_zoom: 1.0,
            ui_zoom: 1.0,
        }
    }
}

impl ViewPrefs {
    /// Reads the preferences from the `view` value; anything missing or of
    /// the wrong type keeps its default.
    pub(crate) fn from_value(value: Option<&Value>) -> ViewPrefs {
        let d = ViewPrefs::default();
        let Some(Value::Object(map)) = value else {
            return d;
        };
        let flag = |k: &str, default: bool| map.get(k).and_then(Value::as_bool).unwrap_or(default);
        let pick = |k: &str, names: &[&str]| {
            map.get(k)
                .and_then(Value::as_str)
                .and_then(|n| names.iter().position(|x| x.eq_ignore_ascii_case(n)))
        };
        ViewPrefs {
            dark_interface: flag("dark_interface", d.dark_interface),
            dark_page: flag("dark_page", d.dark_page),
            dynamics: flag("dynamics", d.dynamics),
            layout: pick("layout", &LAYOUTS)
                .and_then(|i| Layout::ALL.get(i).copied())
                .unwrap_or(d.layout),
            paper: pick("paper", &PAPERS)
                .and_then(|i| Paper::ALL.get(i).copied())
                .unwrap_or(d.paper),
            score_zoom: map
                .get("score_zoom")
                .and_then(Value::as_f64)
                .map(|v| v as f32)
                .unwrap_or(d.score_zoom),
            ui_zoom: map
                .get("ui_zoom")
                .and_then(Value::as_f64)
                .map(|v| v as f32)
                .unwrap_or(d.ui_zoom),
        }
    }

    /// The `view` value for these preferences. Keys a later version added
    /// to `old` are kept.
    pub(crate) fn to_value(self, old: Option<&Value>) -> Value {
        let mut map = match old {
            Some(Value::Object(m)) => m.clone(),
            _ => Map::new(),
        };
        let name = |all: &[&'static str], i: Option<usize>| all.get(i.unwrap_or(0)).copied();
        map.insert("dark_interface".into(), json!(self.dark_interface));
        map.insert("dark_page".into(), json!(self.dark_page));
        map.insert("dynamics".into(), json!(self.dynamics));
        map.insert(
            "layout".into(),
            json!(name(
                &LAYOUTS,
                Layout::ALL.iter().position(|l| *l == self.layout)
            )),
        );
        map.insert(
            "paper".into(),
            json!(name(
                &PAPERS,
                Paper::ALL.iter().position(|p| *p == self.paper)
            )),
        );
        map.insert("score_zoom".into(), json!(self.score_zoom));
        map.insert("ui_zoom".into(), json!(self.ui_zoom));
        Value::Object(map)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_a_dark_interface_and_a_white_page() {
        let d = ViewPrefs::from_value(None);
        assert!(d.dark_interface && !d.dark_page && !d.dynamics);
        assert_eq!(ViewPrefs::from_value(Some(&json!("nonsense"))), d);
        assert_eq!(d.score_zoom, 1.0);
        assert_eq!(d.ui_zoom, 1.0);
    }

    #[test]
    fn round_trip_keeps_unknown_keys() {
        let p = ViewPrefs {
            dark_interface: false,
            dark_page: true,
            dynamics: true,
            layout: Layout::Horizontal,
            paper: Paper::Letter,
            score_zoom: 2.0,
            ui_zoom: 1.25,
        };
        let v = p.to_value(Some(&json!({"later": 3})));
        assert_eq!(v["layout"], json!("horizontal"));
        assert_eq!(v["paper"], json!("letter"));
        assert_eq!(v["later"], json!(3));
        assert_eq!(v["score_zoom"], json!(2.0));
        assert_eq!(v["ui_zoom"], json!(1.25));
        assert_eq!(ViewPrefs::from_value(Some(&v)), p);
    }

    #[test]
    fn wrong_values_keep_their_defaults() {
        let p = ViewPrefs::from_value(Some(
            &json!({"dark_page": "yes", "layout": "sideways", "paper": "Letter", "score_zoom": "fast", "ui_zoom": true}),
        ));
        assert!(!p.dark_page);
        assert_eq!(p.layout, Layout::Page);
        assert_eq!(p.paper, Paper::Letter);
        assert_eq!(p.score_zoom, 1.0);
        assert_eq!(p.ui_zoom, 1.0);
    }
}
