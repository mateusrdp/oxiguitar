// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! An SFZ instrument: its regions, each with what it inherits from the
//! `<global>`, `<master>` and `<group>` headers above it; its `<control>`
//! settings (where samples are, controllers' starting values, a key
//! offset); its `<curve>`s; and what it holds that this player doesn't
//! know, counted, so a set that plays badly can say why.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::region::{self, Applied, CCS, Context, Mod, Region, Target};
use super::text::{Item, Text};

/// Curves: the seven built-in ones and those an instrument defines
/// (`<curve>`), each 128 points from a controller's 0 to its 127.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Curves {
    custom: Vec<(u16, Box<[f32; 128]>)>,
}

impl Curves {
    /// The value of curve `index` at `x`, from 0 to 1 (a controller's
    /// value over 127). An index with no curve is the linear one, 0.
    pub(crate) fn at(&self, index: u16, x: f32) -> f32 {
        let x = x.clamp(0.0, 1.0);
        if let Some((_, points)) = self.custom.iter().find(|(i, _)| *i == index) {
            let pos = x * 127.0;
            let i = (pos as usize).min(126);
            let t = pos - i as f32;
            return points[i] + (points[i + 1] - points[i]) * t;
        }
        match index {
            1 => 2.0 * x - 1.0,
            2 => 1.0 - x,
            3 => 1.0 - 2.0 * x,
            4 => x * x,
            5 => x.sqrt(),
            6 => (1.0 - x).sqrt(),
            _ => x,
        }
    }
}

/// A curve's points as written (`vNNN=value`), the rest filled in
/// linearly; `v000` is 0 and `v127` 1 unless given.
fn curve(points: &[(u8, f32)]) -> Box<[f32; 128]> {
    let mut values = Box::new([0.0f32; 128]);
    let mut set = [false; 128];
    values[0] = 0.0;
    values[127] = 1.0;
    set[0] = true;
    set[127] = true;
    for &(i, v) in points {
        values[usize::from(i)] = v;
        set[usize::from(i)] = true;
    }
    let mut left = 0;
    for right in 1..128 {
        if !set[right] {
            continue;
        }
        let span = right - left;
        for k in 1..span {
            let t = k as f32 / span as f32;
            values[left + k] = values[left] + (values[right] - values[left]) * t;
        }
        left = right;
    }
    values
}

/// An instrument read from its `.sfz` file.
#[derive(Clone, Debug)]
pub(crate) struct Instrument {
    /// The `.sfz` file.
    pub(crate) path: PathBuf,
    pub(crate) regions: Vec<Region>,
    /// Every sample file its regions play, each once.
    pub(crate) samples: Vec<PathBuf>,
    pub(crate) curves: Curves,
    /// Each controller's value before anything moves it (`set_ccN`).
    pub(crate) controllers: [f32; CCS],
    /// The key switch that counts as pressed before any is (`sw_default`).
    pub(crate) default_switch: Option<u8>,
    /// Opcodes this player doesn't know, by shape, and how many times
    /// they came.
    pub(crate) unknown: BTreeMap<String, usize>,
    /// What was skipped while reading: missing includes, values that
    /// don't read, headers it doesn't play.
    pub(crate) problems: Vec<String>,
    /// Every file read: the instrument and what it includes.
    pub(crate) files: Vec<PathBuf>,
}

/// The headers whose opcodes regions inherit.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Scope {
    None,
    Control,
    Global,
    Master,
    Group,
    Region,
    Curve,
    /// `<effect>`, `<midi>`, `<sample>`: nothing OxiGuitar plays uses them.
    Other,
}

struct Builder {
    ctx: Context,
    global: Vec<(String, String)>,
    master: Vec<(String, String)>,
    group: Vec<(String, String)>,
    region: Vec<(String, String)>,
    curve_index: Option<u16>,
    curve_points: Vec<(u8, f32)>,
    scope: Scope,
    note_offset: i32,
    octave_offset: i32,
    instrument: Instrument,
}

impl Builder {
    fn problem(&mut self, what: String) {
        if self.instrument.problems.len() < 100 {
            self.instrument.problems.push(what);
        }
    }

    /// Ends the header being read: a region is made, a curve kept.
    fn close(&mut self) {
        match self.scope {
            Scope::Region => {
                let mut r = Region::default();
                let opcodes = self
                    .global
                    .iter()
                    .chain(&self.master)
                    .chain(&self.group)
                    .chain(&self.region);
                let mut problems = Vec::new();
                for (name, value) in opcodes {
                    match r.apply(name, value, &mut self.ctx) {
                        Applied::Taken | Applied::Ignored => {}
                        Applied::Unknown(shape) => {
                            *self.instrument.unknown.entry(shape).or_default() += 1;
                        }
                        Applied::Bad(what) => problems.push(format!("can't read {what}")),
                    }
                }
                for p in problems {
                    self.problem(p);
                }
                if self.instrument.default_switch.is_none() {
                    self.instrument.default_switch = r.sw_default;
                }
                self.instrument.regions.push(r);
                self.region.clear();
            }
            Scope::Curve => {
                if let Some(index) = self.curve_index {
                    let points = curve(&self.curve_points);
                    let curves = &mut self.instrument.curves.custom;
                    curves.retain(|(i, _)| *i != index);
                    curves.push((index, points));
                }
                self.curve_index = None;
                self.curve_points.clear();
            }
            _ => {}
        }
    }

    fn header(&mut self, name: &str) {
        self.close();
        self.scope = match name {
            "control" => Scope::Control,
            "global" => {
                self.global.clear();
                self.master.clear();
                self.group.clear();
                Scope::Global
            }
            "master" => {
                self.master.clear();
                self.group.clear();
                Scope::Master
            }
            "group" => {
                self.group.clear();
                Scope::Group
            }
            "region" => Scope::Region,
            "curve" => Scope::Curve,
            "effect" | "midi" | "sample" => Scope::Other,
            other => {
                self.problem(format!("<{other}> isn't a header it knows"));
                Scope::Other
            }
        };
    }

    fn opcode(&mut self, name: String, value: String) {
        match self.scope {
            Scope::Control => self.control(&name, &value),
            Scope::Global => self.global.push((name, value)),
            Scope::Master => self.master.push((name, value)),
            Scope::Group => self.group.push((name, value)),
            Scope::Region => self.region.push((name, value)),
            Scope::Curve => self.curve_opcode(&name, &value),
            Scope::Other => {}
            Scope::None => self.problem(format!("{name}={value} comes before any header")),
        }
    }

    fn control(&mut self, name: &str, value: &str) {
        let (pattern, numbers) = region::shape(name);
        let cc = numbers
            .first()
            .and_then(|&n| usize::try_from(n).ok())
            .filter(|&n| n < CCS);
        match pattern.as_str() {
            "default_path" => {
                self.ctx.default_path = value.trim().replace('\\', "/");
            }
            "set_cc&" | "set_hdcc&" | "set_realcc&" => match (cc, region::number(value)) {
                (Some(cc), Some(v)) => {
                    let v = if pattern == "set_cc&" { v } else { v * 127.0 };
                    self.instrument.controllers[cc] = v.clamp(0.0, 127.0);
                }
                _ => self.problem(format!("can't read {name}={value}")),
            },
            "note_offset" | "octave_offset" => match region::number(value) {
                Some(v) => {
                    let v = v as i32;
                    if pattern == "note_offset" {
                        self.note_offset = v;
                    } else {
                        self.octave_offset = v;
                    }
                    self.ctx.key_offset = self.note_offset + 12 * self.octave_offset;
                }
                None => self.problem(format!("can't read {name}={value}")),
            },
            "label_cc&" | "label_key&" | "hint_&" => {}
            other => {
                *self
                    .instrument
                    .unknown
                    .entry(format!("<control> {other}"))
                    .or_default() += 1;
            }
        }
    }

    fn curve_opcode(&mut self, name: &str, value: &str) {
        let (pattern, numbers) = region::shape(name);
        match (pattern.as_str(), numbers.first(), region::number(value)) {
            ("curve_index", _, Some(v)) if (0.0..=255.0).contains(&v) => {
                self.curve_index = Some(v as u16);
            }
            ("v&", Some(&n), Some(v)) if n <= 127 => {
                self.curve_points.push((n as u8, v));
            }
            _ => self.problem(format!("can't read {name}={value} in <curve>")),
        }
    }
}

impl Builder {
    fn new(path: &Path) -> Builder {
        Builder {
            ctx: Context {
                base: path.parent().map(Path::to_path_buf).unwrap_or_default(),
                ..Context::default()
            },
            global: Vec::new(),
            master: Vec::new(),
            group: Vec::new(),
            region: Vec::new(),
            curve_index: None,
            curve_points: Vec::new(),
            scope: Scope::None,
            note_offset: 0,
            octave_offset: 0,
            instrument: Instrument {
                path: path.to_path_buf(),
                regions: Vec::new(),
                samples: Vec::new(),
                curves: Curves { custom: Vec::new() },
                controllers: starting_controllers(),
                default_switch: None,
                unknown: BTreeMap::new(),
                problems: Vec::new(),
                files: Vec::new(),
            },
        }
    }
}

/// Controllers before an instrument sets any: 0, but for volume (7) at
/// 100, pan (10) in the middle and expression (11) full, as General MIDI
/// and sfizz have them.
fn starting_controllers() -> [f32; CCS] {
    let mut c = [0.0; CCS];
    c[7] = 100.0;
    c[10] = 64.0;
    c[11] = 127.0;
    c
}

/// Builds the instrument at `path` from its read `text`.
pub(crate) fn build(path: &Path, text: Text) -> Instrument {
    let mut b = Builder::new(path);
    b.instrument.files = text.files;
    for p in text.problems {
        b.problem(p);
    }
    for item in text.items {
        match item {
            Item::Header(name) => b.header(&name),
            Item::Opcode(name, value) => b.opcode(name, value),
        }
    }
    b.close();
    let mut instrument = b.instrument;
    instrument.samples = b.ctx.samples;
    implicit_controllers(&mut instrument.regions);
    instrument
}

/// Volume (7), pan (10) and expression (11) move every region unless the
/// instrument uses those controllers itself, as in sfizz: 7 and 11 the
/// amplitude through the square curve (4), 10 the pan through the
/// bipolar one (1).
fn implicit_controllers(regions: &mut [Region]) {
    let uses = |cc: u16| regions.iter().any(|r| r.mods.iter().any(|m| m.cc == cc));
    let defaults = [
        (7, Target::Amplitude, 4),
        (10, Target::Pan, 1),
        (11, Target::Amplitude, 4),
    ];
    let wanted: Vec<_> = defaults
        .into_iter()
        .filter(|&(cc, _, _)| !uses(cc))
        .collect();
    for r in regions.iter_mut() {
        for &(cc, target, curve) in &wanted {
            r.mods.push(Mod {
                target,
                cc,
                depth: 100.0,
                curve,
            });
        }
    }
}

/// Reads the instrument at `path` from the disk.
pub(crate) fn read(path: &Path) -> Result<Instrument, String> {
    Ok(build(path, super::text::read(path)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sfz::region::{KeyCenter, Source, Trigger};
    use crate::sfz::text::read_with;

    fn instrument(text: &str) -> Instrument {
        let path = PathBuf::from("/k/Programs/i.sfz");
        let bytes = text.as_bytes().to_vec();
        let load = move |p: &Path| {
            if p == Path::new("/k/Programs/i.sfz") {
                Ok(bytes.clone())
            } else {
                Err(std::io::Error::from(std::io::ErrorKind::NotFound))
            }
        };
        build(&path, read_with(&path, &load).unwrap())
    }

    #[test]
    fn regions_inherit_from_their_headers_and_headers_reset_below_them() {
        let i = instrument(
            "<global> ampeg_release=0.25 volume=2\n\
             <master> sw_last=36\n\
             <group> seq_length=2 trigger=release\n\
             <region> sample=a.wav key=40 seq_position=1\n\
             <region> sample=b.wav key=40 seq_position=2 volume=-1\n\
             <master> sw_last=37\n\
             <region> sample=a.wav key=41\n\
             <global>\n\
             <region> sample=c.wav",
        );
        assert_eq!(i.regions.len(), 4);
        let [a, b, c, d] = [&i.regions[0], &i.regions[1], &i.regions[2], &i.regions[3]];
        assert_eq!(
            (a.ampeg.release, a.volume, a.sw_last),
            (0.25, 2.0, Some((36, 36)))
        );
        assert_eq!(
            (a.seq_length, a.seq_position, a.trigger),
            (2, 1, Trigger::Release)
        );
        assert_eq!((b.seq_position, b.volume), (2, -1.0));
        // A new master drops the group's opcodes.
        assert_eq!(
            (c.sw_last, c.seq_length, c.trigger),
            (Some((37, 37)), 1, Trigger::Attack)
        );
        assert_eq!((c.lokey, c.keycenter), (41, KeyCenter::Key(41)));
        // A new global drops everything above.
        assert_eq!((d.volume, d.sw_last, d.ampeg.release), (0.0, None, 0.001));
        assert_eq!(
            i.samples,
            [
                PathBuf::from("/k/Programs/a.wav"),
                PathBuf::from("/k/Programs/b.wav"),
                PathBuf::from("/k/Programs/c.wav")
            ]
        );
        assert_eq!(
            [a.source, c.source, d.source],
            [
                Some(Source::File(0)),
                Some(Source::File(0)),
                Some(Source::File(2))
            ]
        );
    }

    #[test]
    fn control_sets_controllers_paths_and_offsets() {
        let i = instrument(
            "<control> default_path=../Samples/ set_cc27=31 set_hdcc1=0.5 octave_offset=-1 note_offset=2 label_cc27=Release\n\
             <region> sample=x.wav key=60 sw_default=36",
        );
        assert_eq!(i.samples, [PathBuf::from("/k/Samples/x.wav")]);
        assert_eq!(i.controllers[27], 31.0);
        assert_eq!(i.controllers[1], 63.5);
        assert_eq!(i.controllers[7], 100.0);
        assert_eq!(i.controllers[11], 127.0);
        assert_eq!(i.regions[0].lokey, 50);
        assert_eq!(i.default_switch, Some(26));
        assert!(i.unknown.is_empty(), "{:?}", i.unknown);
    }

    #[test]
    fn curves_built_in_and_written() {
        let i = instrument(
            "<curve> curve_index=7 v063=1\n<curve> curve_index=8 v000=1 v127=0\n<region> sample=x.wav",
        );
        let c = &i.curves;
        assert_eq!(c.at(0, 0.5), 0.5);
        assert_eq!(c.at(1, 0.0), -1.0);
        assert_eq!(c.at(4, 0.5), 0.25);
        assert_eq!(c.at(7, 0.0), 0.0);
        assert_eq!(c.at(7, 63.0 / 127.0), 1.0);
        assert!((c.at(7, 31.5 / 127.0) - 0.5).abs() < 1e-6);
        assert_eq!(c.at(7, 1.0), 1.0);
        assert_eq!(c.at(8, 0.0), 1.0);
        assert_eq!(c.at(8, 1.0), 0.0);
        assert_eq!(c.at(99, 0.3), 0.3);
    }

    #[test]
    fn volume_pan_and_expression_move_regions_unless_the_instrument_uses_them() {
        let i = instrument("<region> sample=x.wav");
        let ccs: Vec<u16> = i.regions[0].mods.iter().map(|m| m.cc).collect();
        assert_eq!(ccs, [7, 10, 11]);
        let i = instrument("<region> sample=x.wav <region> sample=y.wav amplitude_oncc7=50");
        let ccs: Vec<u16> = i.regions[0].mods.iter().map(|m| m.cc).collect();
        assert_eq!(ccs, [10, 11]);
    }

    #[test]
    fn what_it_doesnt_know_is_counted() {
        let i = instrument(
            "<control> foo=1\n<global> eq1_gain=3\n<region> sample=x.wav eq2_gain=1 lokey=zz\n<region> sample=y.wav\n<effect> type=reverb\n<weird>",
        );
        assert_eq!(i.unknown.get("eq&_gain"), Some(&3));
        assert_eq!(i.unknown.get("<control> foo"), Some(&1));
        assert_eq!(i.problems.len(), 2, "{:?}", i.problems);
    }

    /// Reads every instrument under `OXIGUITAR_SFZ_DIR` (sets unpacked
    /// there, samples or not) and prints what this player doesn't know:
    /// `OXIGUITAR_SFZ_DIR=… cargo test -p oxiguitar-soundfont-plugin
    /// survey -- --ignored --nocapture`.
    #[test]
    #[ignore = "needs sound sets on the disk"]
    fn survey() {
        let Some(dir) = std::env::var_os("OXIGUITAR_SFZ_DIR") else {
            return;
        };
        let found = crate::sets::find(Some(Path::new(&dir)), &[]);
        let mut unknown: BTreeMap<String, (usize, usize)> = BTreeMap::new();
        let (mut regions, mut instruments) = (0, 0);
        for f in found.iter().filter(|f| f.kind == crate::sets::Kind::Sfz) {
            let i = match read(&f.path) {
                Ok(i) => i,
                Err(e) => {
                    println!("{e}");
                    continue;
                }
            };
            instruments += 1;
            regions += i.regions.len();
            for (shape, n) in &i.unknown {
                let e = unknown.entry(shape.clone()).or_default();
                e.0 += n;
                e.1 += 1;
            }
            let shown: Vec<_> = i
                .problems
                .iter()
                .filter(|p| !p.starts_with("cannot include"))
                .take(3)
                .collect();
            if !shown.is_empty() {
                println!("{}: {shown:?}", f.path.display());
            }
        }
        println!("{instruments} instruments, {regions} regions");
        for (shape, (n, files)) in unknown {
            println!("{n:7} in {files:3} instruments  {shape}");
        }
    }
}
