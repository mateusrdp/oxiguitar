// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! A region's opcodes as values, with the defaults and meanings of sfizz,
//! the format's reference player (sfzformat.com documents most of them):
//! what sample plays and how, which notes and velocities trigger it, its
//! pitch, level, envelopes, filter and LFOs, and how controllers move them.
//!
//! Controllers. OxiGuitar sends none of its own while playing, so a
//! controller keeps the value the instrument gives it (`set_ccN`), or one
//! a mapping file chooses for an articulation; a region's controller
//! opcodes (`amplitude_oncc7=…`, `locc64=…`, `xfin_locc1=…`) are kept as
//! they are written and worked out when a note starts.

use std::path::{Path, PathBuf};

/// Controllers 0 to 127, then the extended ones sfizz and ARIA define:
/// 128 pitch bend, 129 channel and 130 polyphonic aftertouch, 131 note-on
/// and 132 note-off velocity, 133 the key, 134 key gate, 135 a unipolar and
/// 136 a bipolar random number, 137 alternating, 140 and 141 the key's
/// distance from the previous note; and up to 1023, controllers an
/// instrument uses for its own settings (ARIA's, as Karoryfer's sets do).
pub(crate) const CCS: usize = 1024;

/// Extended controllers, by number.
pub(crate) mod cc {
    pub(crate) const VELOCITY: u16 = 131;
    pub(crate) const KEY: u16 = 133;
    pub(crate) const GATE: u16 = 134;
    pub(crate) const RANDOM: u16 = 135;
    pub(crate) const BIPOLAR_RANDOM: u16 = 136;
    pub(crate) const ALTERNATE: u16 = 137;
    pub(crate) const KEY_DELTA: u16 = 140;
    pub(crate) const ABSOLUTE_KEY_DELTA: u16 = 141;
}

/// What starts a region.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Trigger {
    /// A key pressed.
    Attack,
    /// A key let go: release samples.
    Release,
    /// A key pressed while no other is.
    First,
    /// A key pressed while another is.
    Legato,
    /// A key let go, even while the sustain pedal holds it.
    ReleaseKey,
}

/// How a sample loops.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LoopMode {
    /// Plays to its end, or until the note's release ends.
    NoLoop,
    /// Plays to its end whatever the note does (drums).
    OneShot,
    /// Loops until the release ends.
    Continuous,
    /// Loops until the note is let go, then plays on to the end.
    Sustain,
}

/// How a region stops a note it chokes (`off_by`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OffMode {
    /// Within a few milliseconds.
    Fast,
    /// Through its release.
    Normal,
    /// Over `off_time`.
    Time,
}

/// A crossfade's curve.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Crossfade {
    /// Equal power.
    Power,
    /// Linear in amplitude.
    Gain,
}

/// A filter's kind (`fil_type`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FilterKind {
    LowPass1,
    LowPass2,
    LowPass4,
    LowPass6,
    HighPass1,
    HighPass2,
    HighPass4,
    HighPass6,
    BandPass1,
    BandPass2,
    BandReject2,
    LowShelf,
    HighShelf,
    Peak,
}

impl FilterKind {
    fn parse(v: &str) -> Option<FilterKind> {
        Some(match v.trim() {
            "lpf_1p" => FilterKind::LowPass1,
            "lpf_2p" | "lpf_2p_sv" => FilterKind::LowPass2,
            "lpf_4p" => FilterKind::LowPass4,
            "lpf_6p" => FilterKind::LowPass6,
            "hpf_1p" => FilterKind::HighPass1,
            "hpf_2p" | "hpf_2p_sv" => FilterKind::HighPass2,
            "hpf_4p" => FilterKind::HighPass4,
            "hpf_6p" => FilterKind::HighPass6,
            "bpf_1p" => FilterKind::BandPass1,
            "bpf_2p" | "bpf_2p_sv" => FilterKind::BandPass2,
            "brf_2p" | "brf_2p_sv" | "brf_1p" => FilterKind::BandReject2,
            "lsh" => FilterKind::LowShelf,
            "hsh" => FilterKind::HighShelf,
            "peq" | "pink" => FilterKind::Peak,
            _ => return None,
        })
    }
}

/// An LFO's shape (`lfoN_wave`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Wave {
    Triangle,
    Sine,
    Pulse75,
    Square,
    Pulse25,
    Pulse12,
    Ramp,
    Saw,
    /// A new random value each cycle.
    Random,
}

impl Wave {
    fn of(v: i64) -> Option<Wave> {
        Some(match v {
            0 => Wave::Triangle,
            1 => Wave::Sine,
            2 => Wave::Pulse75,
            3 => Wave::Square,
            4 => Wave::Pulse25,
            5 => Wave::Pulse12,
            6 => Wave::Ramp,
            7 => Wave::Saw,
            12 => Wave::Random,
            _ => return None,
        })
    }
}

/// Which envelope.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EgKind {
    Amp,
    Filter,
    Pitch,
}

/// An envelope's stage or depth.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EgParam {
    Delay,
    Start,
    Attack,
    Hold,
    Decay,
    Sustain,
    Release,
    Depth,
}

/// What an LFO (SFZ 2's numbered ones) has and moves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LfoParam {
    Freq,
    Delay,
    Fade,
    Phase,
    Pitch,
    Volume,
    Amplitude,
    Pan,
    Cutoff,
}

/// SFZ 1's three LFOs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OldLfo {
    Amp,
    Filter,
    Pitch,
}

/// What a controller moves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Target {
    /// Percent, multiplying.
    Amplitude,
    /// Decibels.
    Volume,
    /// Percent, −100 left to 100 right.
    Pan,
    Width,
    Position,
    /// Cents.
    Tune,
    /// Frames into the sample.
    Offset,
    /// Seconds before the region sounds.
    Delay,
    /// Percent added to `amp_veltrack`.
    VelTrack,
    /// Seconds, percent or cents, by stage.
    Eg(EgKind, EgParam),
    /// Cents.
    Cutoff,
    /// Decibels.
    Resonance,
    /// The numbered LFO's parameter.
    Lfo(u8, LfoParam),
    /// SFZ 1's LFO's depth.
    OldLfoDepth(OldLfo),
    /// SFZ 1's LFO's rate.
    OldLfoFreq(OldLfo),
}

/// A controller moving something: `depth` at the controller's top, through
/// curve `curve` (0, linear, unless set).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Mod {
    pub(crate) target: Target,
    pub(crate) cc: u16,
    pub(crate) depth: f32,
    pub(crate) curve: u16,
}

/// A controller crossfading a region in or out (`xfin_loccN` and the like).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct CcFade {
    pub(crate) cc: u16,
    /// Silent at or below the first, full from the second (0, 0: always
    /// full).
    pub(crate) fade_in: (f32, f32),
    /// Full up to the first, silent from the second (127, 127: always
    /// full).
    pub(crate) fade_out: (f32, f32),
}

/// An envelope: times in seconds, levels in percent, depth in cents.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Eg {
    pub(crate) delay: f32,
    pub(crate) start: f32,
    pub(crate) attack: f32,
    pub(crate) hold: f32,
    pub(crate) decay: f32,
    pub(crate) sustain: f32,
    pub(crate) release: f32,
    pub(crate) depth: f32,
    /// How velocity adds to each, at full velocity.
    pub(crate) vel2_delay: f32,
    pub(crate) vel2_attack: f32,
    pub(crate) vel2_hold: f32,
    pub(crate) vel2_decay: f32,
    pub(crate) vel2_sustain: f32,
    pub(crate) vel2_release: f32,
    pub(crate) vel2_depth: f32,
    /// Curves of the attack, decay and release (0 linear; positive
    /// bends one way, negative the other).
    pub(crate) attack_shape: f32,
    pub(crate) decay_shape: f32,
    pub(crate) release_shape: f32,
}

impl Eg {
    const fn new(release: f32) -> Eg {
        Eg {
            delay: 0.0,
            start: 0.0,
            attack: 0.0,
            hold: 0.0,
            decay: 0.0,
            sustain: 100.0,
            release,
            depth: 0.0,
            vel2_delay: 0.0,
            vel2_attack: 0.0,
            vel2_hold: 0.0,
            vel2_decay: 0.0,
            vel2_sustain: 0.0,
            vel2_release: 0.0,
            vel2_depth: 0.0,
            attack_shape: 0.0,
            decay_shape: 0.0,
            release_shape: 0.0,
        }
    }
}

/// One of SFZ 2's numbered LFOs (`lfo01_…`): hertz, seconds, cycles,
/// and what it moves (cents, decibels, percent).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Lfo {
    pub(crate) index: u8,
    pub(crate) freq: f32,
    pub(crate) delay: f32,
    pub(crate) fade: f32,
    pub(crate) phase: f32,
    pub(crate) wave: Wave,
    pub(crate) pitch: f32,
    pub(crate) volume: f32,
    pub(crate) amplitude: f32,
    pub(crate) pan: f32,
    pub(crate) cutoff: f32,
}

impl Lfo {
    const fn new(index: u8) -> Lfo {
        Lfo {
            index,
            freq: 0.0,
            delay: 0.0,
            fade: 0.0,
            phase: 0.0,
            wave: Wave::Triangle,
            pitch: 0.0,
            volume: 0.0,
            amplitude: 0.0,
            pan: 0.0,
            cutoff: 0.0,
        }
    }
}

/// One of SFZ 1's LFOs (`pitchlfo_…`, `amplfo_…`, `fillfo_…`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct SimpleLfo {
    pub(crate) depth: f32,
    pub(crate) freq: f32,
    pub(crate) delay: f32,
    pub(crate) fade: f32,
}

/// What a region plays.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Source {
    /// The instrument's sample of this index.
    File(usize),
    /// `*sine`: a sine at the region's pitch.
    Sine,
    /// `*silence`.
    Silence,
    /// `*noise`: white noise.
    Noise,
}

/// The key a region's sample sounds at unchanged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum KeyCenter {
    Key(u8),
    /// The sample's own unity note (its `smpl` chunk), else 60.
    FromSample,
}

/// A region, with everything it inherits from its group, master and
/// global headers.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Region {
    // What plays.
    pub(crate) source: Option<Source>,
    pub(crate) offset: i64,
    pub(crate) offset_random: i64,
    /// The last frame played, if not the sample's last.
    pub(crate) end: Option<i64>,
    pub(crate) loop_mode: Option<LoopMode>,
    pub(crate) loop_start: Option<i64>,
    /// The loop's last frame.
    pub(crate) loop_end: Option<i64>,
    pub(crate) reverse: bool,
    pub(crate) delay: f32,
    pub(crate) delay_random: f32,
    // When it plays.
    pub(crate) lokey: u8,
    pub(crate) hikey: u8,
    pub(crate) lovel: u8,
    pub(crate) hivel: u8,
    /// Plays when a random number in [0, 1) falls in [lorand, hirand).
    pub(crate) lorand: f32,
    pub(crate) hirand: f32,
    pub(crate) seq_length: u32,
    pub(crate) seq_position: u32,
    pub(crate) trigger: Trigger,
    /// Whether MIDI channel 1, the only one OxiGuitar plays, is in the
    /// region's `lochan`..`hichan`.
    pub(crate) channel: (u8, u8),
    /// Controller ranges the region needs (`loccN`, `hiccN`).
    pub(crate) cc_ranges: Vec<(u16, f32, f32)>,
    pub(crate) lobend: f32,
    pub(crate) hibend: f32,
    /// Started by a controller moving (`on_loccN`, `on_hiccN`), never by
    /// a key: OxiGuitar moves none, so such a region never plays.
    pub(crate) cc_triggered: bool,
    pub(crate) sw_lokey: Option<u8>,
    pub(crate) sw_hikey: Option<u8>,
    /// The key switch range that must have been the last pressed.
    pub(crate) sw_last: Option<(u8, u8)>,
    pub(crate) sw_default: Option<u8>,
    pub(crate) sw_down: Option<u8>,
    pub(crate) sw_up: Option<u8>,
    pub(crate) sw_previous: Option<u8>,
    pub(crate) group: i64,
    pub(crate) off_by: Option<i64>,
    pub(crate) off_mode: OffMode,
    pub(crate) off_time: f32,
    pub(crate) polyphony: Option<u32>,
    pub(crate) note_polyphony: Option<u32>,
    pub(crate) note_selfmask: bool,
    // Pitch.
    pub(crate) keycenter: KeyCenter,
    /// Cents a key.
    pub(crate) pitch_keytrack: f32,
    pub(crate) pitch_veltrack: f32,
    pub(crate) pitch_random: f32,
    pub(crate) transpose: i32,
    /// Cents.
    pub(crate) tune: f32,
    pub(crate) bend_up: f32,
    pub(crate) bend_down: f32,
    // Level.
    /// Decibels: the region's, and ARIA's for its group, master and
    /// global headers, which add up.
    pub(crate) volume: f32,
    pub(crate) group_volume: f32,
    pub(crate) master_volume: f32,
    pub(crate) global_volume: f32,
    pub(crate) amplitude: f32,
    pub(crate) pan: f32,
    pub(crate) width: f32,
    pub(crate) position: f32,
    pub(crate) amp_keycenter: u8,
    pub(crate) amp_keytrack: f32,
    pub(crate) amp_veltrack: f32,
    /// `amp_velcurve_N`: points of the velocity curve.
    pub(crate) velcurve: Vec<(u8, f32)>,
    pub(crate) amp_random: f32,
    /// Decibels a second the release sample loses for each second the key
    /// was held.
    pub(crate) rt_decay: f32,
    pub(crate) xfin_vel: (u8, u8),
    pub(crate) xfout_vel: (u8, u8),
    pub(crate) xfin_key: (u8, u8),
    pub(crate) xfout_key: (u8, u8),
    pub(crate) xf_velcurve: Crossfade,
    pub(crate) xf_keycurve: Crossfade,
    pub(crate) xf_cccurve: Crossfade,
    pub(crate) cc_fades: Vec<CcFade>,
    pub(crate) ampeg: Eg,
    pub(crate) fileg: Eg,
    pub(crate) pitcheg: Eg,
    // Filter.
    pub(crate) filter: Option<FilterKind>,
    /// Hertz; no filter without it.
    pub(crate) cutoff: Option<f32>,
    pub(crate) resonance: f32,
    pub(crate) fil_keytrack: f32,
    pub(crate) fil_keycenter: u8,
    pub(crate) fil_veltrack: f32,
    pub(crate) fil_random: f32,
    // LFOs.
    pub(crate) lfos: Vec<Lfo>,
    pub(crate) amplfo: SimpleLfo,
    pub(crate) fillfo: SimpleLfo,
    pub(crate) pitchlfo: SimpleLfo,
    pub(crate) mods: Vec<Mod>,
}

impl Default for Region {
    fn default() -> Region {
        Region {
            source: None,
            offset: 0,
            offset_random: 0,
            end: None,
            loop_mode: None,
            loop_start: None,
            loop_end: None,
            reverse: false,
            delay: 0.0,
            delay_random: 0.0,
            lokey: 0,
            hikey: 127,
            lovel: 0,
            hivel: 127,
            lorand: 0.0,
            hirand: 1.0,
            seq_length: 1,
            seq_position: 1,
            trigger: Trigger::Attack,
            channel: (1, 16),
            cc_ranges: Vec::new(),
            lobend: -8192.0,
            hibend: 8192.0,
            cc_triggered: false,
            sw_lokey: None,
            sw_hikey: None,
            sw_last: None,
            sw_default: None,
            sw_down: None,
            sw_up: None,
            sw_previous: None,
            group: 0,
            off_by: None,
            off_mode: OffMode::Fast,
            off_time: 0.006,
            polyphony: None,
            note_polyphony: None,
            note_selfmask: true,
            keycenter: KeyCenter::Key(60),
            pitch_keytrack: 100.0,
            pitch_veltrack: 0.0,
            pitch_random: 0.0,
            transpose: 0,
            tune: 0.0,
            bend_up: 200.0,
            bend_down: -200.0,
            volume: 0.0,
            group_volume: 0.0,
            master_volume: 0.0,
            global_volume: 0.0,
            amplitude: 100.0,
            pan: 0.0,
            width: 100.0,
            position: 0.0,
            amp_keycenter: 60,
            amp_keytrack: 0.0,
            amp_veltrack: 100.0,
            velcurve: Vec::new(),
            amp_random: 0.0,
            rt_decay: 0.0,
            xfin_vel: (0, 0),
            xfout_vel: (127, 127),
            xfin_key: (0, 0),
            xfout_key: (127, 127),
            xf_velcurve: Crossfade::Power,
            xf_keycurve: Crossfade::Power,
            xf_cccurve: Crossfade::Power,
            cc_fades: Vec::new(),
            ampeg: Eg::new(0.001),
            fileg: Eg::new(0.0),
            pitcheg: Eg::new(0.0),
            filter: None,
            cutoff: None,
            resonance: 0.0,
            fil_keytrack: 0.0,
            fil_keycenter: 60,
            fil_veltrack: 0.0,
            fil_random: 0.0,
            lfos: Vec::new(),
            amplfo: SimpleLfo::default(),
            fillfo: SimpleLfo::default(),
            pitchlfo: SimpleLfo::default(),
            mods: Vec::new(),
        }
    }
}

/// What applying an opcode came to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Applied {
    /// Taken.
    Taken,
    /// Known and deliberately ignored (labels, effects sends, the sustain
    /// pedal's settings: nothing OxiGuitar plays uses them).
    Ignored,
    /// An opcode this player doesn't know: counted by its shape.
    Unknown(String),
    /// A known opcode with a value it can't read.
    Bad(String),
}

/// What opcodes need from the instrument as they are applied: where
/// samples are, its key offset, and its list of sample files.
#[derive(Debug, Default)]
pub(crate) struct Context {
    /// The top file's folder.
    pub(crate) base: PathBuf,
    /// `default_path`, as written.
    pub(crate) default_path: String,
    /// `note_offset` plus 12 × `octave_offset`.
    pub(crate) key_offset: i32,
    /// Every sample file, each once; `Source::File` holds the index.
    pub(crate) samples: Vec<PathBuf>,
}

impl Context {
    fn sample(&mut self, value: &str) -> Option<Source> {
        let value = value.trim();
        match value {
            "" => return None,
            "*sine" => return Some(Source::Sine),
            "*silence" => return Some(Source::Silence),
            "*noise" => return Some(Source::Noise),
            _ => {}
        }
        let written = format!("{}{}", self.default_path, value).replace('\\', "/");
        let path = normal(&self.base.join(written));
        let index = match self.samples.iter().position(|p| *p == path) {
            Some(i) => i,
            None => {
                self.samples.push(path);
                self.samples.len() - 1
            }
        };
        Some(Source::File(index))
    }

    fn key(&self, value: &str) -> Option<u8> {
        let k = key(value)? + self.key_offset;
        Some(k.clamp(0, 127) as u8)
    }
}

/// `path` with its `.` and `..` worked out, without asking the disk.
pub(crate) fn normal(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                if !out.pop() {
                    out.push("..");
                }
            }
            other => out.push(other),
        }
    }
    out
}

/// A key: a MIDI number or a note name (`c4` is 60, `c#4` and `db4` 61).
pub(crate) fn key(value: &str) -> Option<i32> {
    let v = value.trim();
    if let Ok(n) = v.parse::<i32>() {
        return Some(n);
    }
    let mut chars = v.chars();
    let letter = chars.next()?.to_ascii_lowercase();
    let mut semitone = match letter {
        'c' => 0,
        'd' => 2,
        'e' => 4,
        'f' => 5,
        'g' => 7,
        'a' => 9,
        'b' => 11,
        _ => return None,
    };
    let rest = chars.as_str();
    let rest = if let Some(r) = rest.strip_prefix('#') {
        semitone += 1;
        r
    } else if let Some(r) = rest.strip_prefix('b') {
        semitone -= 1;
        r
    } else {
        rest
    };
    let octave: i32 = rest.parse().ok()?;
    Some((octave + 1) * 12 + semitone)
}

/// The number at the start of `value`: players read `0.3s` as 0.3 and
/// `-1.6.` as −1.6.
pub(crate) fn number(value: &str) -> Option<f32> {
    let v = value.trim();
    if let Ok(n) = v.parse::<f32>() {
        return n.is_finite().then_some(n);
    }
    let end = v
        .char_indices()
        .find(|&(i, c)| !(c.is_ascii_digit() || c == '.' || ((c == '-' || c == '+') && i == 0)))
        .map_or(v.len(), |(i, _)| i);
    (1..=end)
        .rev()
        .find_map(|e| v[..e].parse::<f32>().ok())
        .filter(|n| n.is_finite())
}

fn integer(value: &str) -> Option<i64> {
    let v = value.trim();
    v.parse::<i64>()
        .ok()
        .or_else(|| number(v).map(|n| n as i64))
}

/// An opcode's name with each run of digits replaced by `&`, and the
/// numbers: `lfo02_pitch_oncc113` is `lfo&_pitch_oncc&` with 2 and 113.
pub(crate) fn shape(name: &str) -> (String, Vec<u32>) {
    let mut pattern = String::with_capacity(name.len());
    let mut numbers = Vec::new();
    let mut digits = String::new();
    for c in name.chars() {
        if c.is_ascii_digit() {
            digits.push(c);
            continue;
        }
        if !digits.is_empty() {
            pattern.push('&');
            numbers.push(digits.parse().unwrap_or(u32::MAX));
            digits.clear();
        }
        pattern.push(c.to_ascii_lowercase());
    }
    if !digits.is_empty() {
        pattern.push('&');
        numbers.push(digits.parse().unwrap_or(u32::MAX));
    }
    (pattern, numbers)
}

/// How a controller opcode's name ends.
enum CcPart {
    Depth,
    Curve,
    /// Smoothing and stepping: accepted, not used (OxiGuitar moves no
    /// controller while a note sounds).
    Ignored,
}

/// `name`'s target and part, for controller opcodes: `amplitude_oncc7`,
/// `ampeg_attackcc1` (SFZ 1), `pan_curvecc10`, `lfo01_pitch_smoothcc1`.
fn controller(pattern: &str, numbers: &[u32]) -> Option<(Target, CcPart)> {
    let (stem, part) = if let Some(s) = pattern.strip_suffix("_oncc&") {
        (s, CcPart::Depth)
    } else if let Some(s) = pattern.strip_suffix("_curvecc&") {
        (s, CcPart::Curve)
    } else if let Some(s) = pattern
        .strip_suffix("_smoothcc&")
        .or_else(|| pattern.strip_suffix("_stepcc&"))
    {
        (s, CcPart::Ignored)
    } else if let Some(s) = pattern.strip_suffix("_cc&") {
        (s, CcPart::Depth)
    } else {
        (pattern.strip_suffix("cc&")?, CcPart::Depth)
    };
    let eg = |kind: EgKind, rest: &str| -> Option<Target> {
        let param = match rest {
            "delay" => EgParam::Delay,
            "start" => EgParam::Start,
            "attack" => EgParam::Attack,
            "hold" => EgParam::Hold,
            "decay" => EgParam::Decay,
            "sustain" => EgParam::Sustain,
            "release" => EgParam::Release,
            "depth" if kind != EgKind::Amp => EgParam::Depth,
            _ => return None,
        };
        Some(Target::Eg(kind, param))
    };
    let target = match stem {
        "amplitude" => Target::Amplitude,
        "volume" | "gain" => Target::Volume,
        "pan" => Target::Pan,
        "width" => Target::Width,
        "position" => Target::Position,
        "tune" | "pitch" => Target::Tune,
        "offset" => Target::Offset,
        "delay" => Target::Delay,
        "amp_veltrack" => Target::VelTrack,
        "cutoff" => Target::Cutoff,
        "resonance" => Target::Resonance,
        // The first filter's, numbered (`cutoff1`).
        "cutoff&" if numbers.first() == Some(&1) => Target::Cutoff,
        "resonance&" if numbers.first() == Some(&1) => Target::Resonance,
        "pitchlfo_depth" => Target::OldLfoDepth(OldLfo::Pitch),
        "amplfo_depth" => Target::OldLfoDepth(OldLfo::Amp),
        "fillfo_depth" => Target::OldLfoDepth(OldLfo::Filter),
        "pitchlfo_freq" => Target::OldLfoFreq(OldLfo::Pitch),
        "amplfo_freq" => Target::OldLfoFreq(OldLfo::Amp),
        "fillfo_freq" => Target::OldLfoFreq(OldLfo::Filter),
        s => {
            if let Some(rest) = s.strip_prefix("ampeg_") {
                eg(EgKind::Amp, rest)?
            } else if let Some(rest) = s.strip_prefix("fileg_") {
                eg(EgKind::Filter, rest)?
            } else if let Some(rest) = s.strip_prefix("pitcheg_") {
                eg(EgKind::Pitch, rest)?
            } else {
                let rest = s.strip_prefix("lfo&_")?;
                let index = u8::try_from(*numbers.first()?).ok()?;
                let param = match rest {
                    "freq" => LfoParam::Freq,
                    "delay" => LfoParam::Delay,
                    "fade" => LfoParam::Fade,
                    "phase" => LfoParam::Phase,
                    "pitch" => LfoParam::Pitch,
                    "volume" => LfoParam::Volume,
                    "amplitude" => LfoParam::Amplitude,
                    "pan" => LfoParam::Pan,
                    "cutoff" | "cutoff&" => LfoParam::Cutoff,
                    _ => return None,
                };
                Target::Lfo(index, param)
            }
        }
    };
    Some((target, part))
}

/// Opcodes known and deliberately ignored.
fn ignored(pattern: &str) -> bool {
    matches!(
        pattern,
        "sw_label"
            | "label_cc&"
            | "label_key&"
            | "region_label"
            | "group_label"
            | "master_label"
            | "global_label"
            | "sw_vel"
            | "effect&"
            | "output"
            | "count"
            | "bend_smooth"
            | "bend_step"
            | "sample_quality"
            | "ampeg_dynamic"
            | "fileg_dynamic"
            | "pitcheg_dynamic"
            | "rt_dead"
            | "sustain_sw"
            | "sostenuto_sw"
            | "sustain_cc"
            | "sustain_lo"
            | "sostenuto_cc"
            | "sostenuto_lo"
            | "loop_crossfade"
            | "hint_&"
            | "image"
            | "image_controls"
            | "lfo&_freq_lfo&_oncc&"
            | "lfo&_freq_lfo&"
            | "off_shape"
            | "phase"
    )
}

impl Region {
    /// The modulation of `target` by `cc`, made if it isn't there yet.
    fn modulation(&mut self, target: Target, cc: u16) -> &mut Mod {
        let i = match self
            .mods
            .iter()
            .position(|m| m.target == target && m.cc == cc)
        {
            Some(i) => i,
            None => {
                self.mods.push(Mod {
                    target,
                    cc,
                    depth: 0.0,
                    curve: 0,
                });
                self.mods.len() - 1
            }
        };
        &mut self.mods[i]
    }

    fn lfo(&mut self, index: u8) -> &mut Lfo {
        let i = match self.lfos.iter().position(|l| l.index == index) {
            Some(i) => i,
            None => {
                self.lfos.push(Lfo::new(index));
                self.lfos.len() - 1
            }
        };
        &mut self.lfos[i]
    }

    fn cc_fade(&mut self, cc: u16) -> &mut CcFade {
        let i = match self.cc_fades.iter().position(|f| f.cc == cc) {
            Some(i) => i,
            None => {
                self.cc_fades.push(CcFade {
                    cc,
                    fade_in: (0.0, 0.0),
                    fade_out: (127.0, 127.0),
                });
                self.cc_fades.len() - 1
            }
        };
        &mut self.cc_fades[i]
    }

    fn cc_range(&mut self, cc: u16) -> &mut (u16, f32, f32) {
        let i = match self.cc_ranges.iter().position(|r| r.0 == cc) {
            Some(i) => i,
            None => {
                self.cc_ranges.push((cc, 0.0, 127.0));
                self.cc_ranges.len() - 1
            }
        };
        &mut self.cc_ranges[i]
    }

    /// Applies the opcode `name=value`.
    pub(crate) fn apply(&mut self, name: &str, value: &str, ctx: &mut Context) -> Applied {
        let (pattern, numbers) = shape(name);
        let bad = || Applied::Bad(format!("{name}={value}"));
        // Values of each kind; a value that won't read leaves the opcode
        // as it was.
        macro_rules! set {
            ($field:expr, $read:expr) => {
                match $read {
                    Some(v) => {
                        $field = v;
                        Applied::Taken
                    }
                    None => bad(),
                }
            };
        }
        let f = || number(value);
        let i = || integer(value);
        let k = || ctx.key(value);
        let key7 = |v: Option<i64>| v.map(|v| v.clamp(0, 127) as u8);
        let cc_number = |n: Option<&u32>| {
            n.and_then(|&n| u16::try_from(n).ok())
                .filter(|&n| usize::from(n) < CCS)
        };
        match pattern.as_str() {
            // What plays.
            "sample" => match ctx.sample(value) {
                Some(s) => {
                    self.source = Some(s);
                    Applied::Taken
                }
                None => bad(),
            },
            "offset" => set!(self.offset, i()),
            "offset_random" => set!(self.offset_random, i()),
            "end" => set!(self.end, i().map(Some)),
            "loop_mode" | "loopmode" => set!(
                self.loop_mode,
                match value.trim() {
                    "no_loop" => Some(Some(LoopMode::NoLoop)),
                    "one_shot" => Some(Some(LoopMode::OneShot)),
                    "loop_continuous" => Some(Some(LoopMode::Continuous)),
                    "loop_sustain" => Some(Some(LoopMode::Sustain)),
                    _ => None,
                }
            ),
            "loop_start" | "loopstart" => set!(self.loop_start, i().map(Some)),
            "loop_end" | "loopend" => set!(self.loop_end, i().map(Some)),
            "direction" => set!(
                self.reverse,
                match value.trim() {
                    "forward" => Some(false),
                    "reverse" => Some(true),
                    _ => None,
                }
            ),
            "delay" => set!(self.delay, f()),
            "delay_random" => set!(self.delay_random, f()),
            // When it plays.
            "lokey" => set!(self.lokey, k()),
            "hikey" => set!(self.hikey, k()),
            "key" => match k() {
                Some(key) => {
                    self.lokey = key;
                    self.hikey = key;
                    self.keycenter = KeyCenter::Key(key);
                    Applied::Taken
                }
                None => bad(),
            },
            "lovel" => set!(self.lovel, key7(i())),
            "hivel" => set!(self.hivel, key7(i())),
            "lorand" => set!(self.lorand, f()),
            "hirand" => set!(self.hirand, f()),
            "seq_length" => set!(self.seq_length, i().map(|v| v.clamp(1, 100) as u32)),
            "seq_position" => set!(self.seq_position, i().map(|v| v.clamp(1, 100) as u32)),
            "trigger" => set!(
                self.trigger,
                match value.trim() {
                    "attack" => Some(Trigger::Attack),
                    "release" => Some(Trigger::Release),
                    "first" => Some(Trigger::First),
                    "legato" => Some(Trigger::Legato),
                    "release_key" => Some(Trigger::ReleaseKey),
                    _ => None,
                }
            ),
            "lochan" => set!(self.channel.0, key7(i())),
            "hichan" => set!(self.channel.1, key7(i())),
            "locc&" | "lohdcc&" | "hicc&" | "hihdcc&" => {
                let Some(cc) = cc_number(numbers.first()) else {
                    return bad();
                };
                let Some(mut v) = f() else {
                    return bad();
                };
                if pattern.contains("hdcc") {
                    v *= 127.0;
                }
                let range = self.cc_range(cc);
                if pattern.starts_with("lo") {
                    range.1 = v;
                } else {
                    range.2 = v;
                }
                Applied::Taken
            }
            "on_locc&" | "on_hicc&" | "on_lohdcc&" | "on_hihdcc&" => {
                self.cc_triggered = true;
                Applied::Taken
            }
            "lobend" => set!(self.lobend, f()),
            "hibend" => set!(self.hibend, f()),
            "sw_lokey" => set!(self.sw_lokey, k().map(Some)),
            "sw_hikey" => set!(self.sw_hikey, k().map(Some)),
            "sw_last" => set!(self.sw_last, k().map(|k| Some((k, k)))),
            "sw_lolast" => match k() {
                Some(key) => {
                    let hi = self.sw_last.map_or(key, |r| r.1);
                    self.sw_last = Some((key, hi.max(key)));
                    Applied::Taken
                }
                None => bad(),
            },
            "sw_hilast" => match k() {
                Some(key) => {
                    let lo = self.sw_last.map_or(key, |r| r.0);
                    self.sw_last = Some((lo.min(key), key));
                    Applied::Taken
                }
                None => bad(),
            },
            "sw_default" => set!(self.sw_default, k().map(Some)),
            "sw_down" => set!(self.sw_down, k().map(Some)),
            "sw_up" => set!(self.sw_up, k().map(Some)),
            "sw_previous" => set!(self.sw_previous, k().map(Some)),
            "group" | "polyphony_group" => set!(self.group, i()),
            "off_by" | "offby" => set!(self.off_by, i().map(Some)),
            "off_mode" => set!(
                self.off_mode,
                match value.trim() {
                    "fast" => Some(OffMode::Fast),
                    "normal" => Some(OffMode::Normal),
                    "time" => Some(OffMode::Time),
                    _ => None,
                }
            ),
            "off_time" => set!(self.off_time, f()),
            "polyphony" => set!(self.polyphony, i().map(|v| Some(v.max(0) as u32))),
            "note_polyphony" => set!(self.note_polyphony, i().map(|v| Some(v.max(0) as u32))),
            "note_selfmask" => set!(
                self.note_selfmask,
                match value.trim() {
                    "on" => Some(true),
                    "off" => Some(false),
                    _ => None,
                }
            ),
            // Pitch.
            "pitch_keycenter" => {
                if value.trim() == "sample" {
                    self.keycenter = KeyCenter::FromSample;
                    Applied::Taken
                } else {
                    set!(self.keycenter, k().map(KeyCenter::Key))
                }
            }
            "pitch_keytrack" => set!(self.pitch_keytrack, f()),
            "pitch_veltrack" => set!(self.pitch_veltrack, f()),
            "pitch_random" => set!(self.pitch_random, f()),
            "transpose" => set!(self.transpose, i().map(|v| v.clamp(-127, 127) as i32)),
            "tune" | "pitch" => set!(self.tune, f()),
            "bend_up" | "bendup" => set!(self.bend_up, f()),
            "bend_down" | "benddown" => set!(self.bend_down, f()),
            // Level.
            "volume" | "gain" => set!(self.volume, f()),
            "group_volume" => set!(self.group_volume, f()),
            "master_volume" => set!(self.master_volume, f()),
            "global_volume" => set!(self.global_volume, f()),
            "amplitude" => set!(self.amplitude, f()),
            "pan" => set!(self.pan, f()),
            "width" => set!(self.width, f()),
            "position" => set!(self.position, f()),
            "amp_keycenter" => set!(self.amp_keycenter, k()),
            "amp_keytrack" => set!(self.amp_keytrack, f()),
            "amp_veltrack" => set!(self.amp_veltrack, f()),
            "amp_velcurve_&" => {
                let (Some(&n), Some(v)) = (numbers.first(), f()) else {
                    return bad();
                };
                let Ok(n) = u8::try_from(n) else {
                    return bad();
                };
                if n > 127 {
                    return bad();
                }
                self.velcurve.retain(|p| p.0 != n);
                self.velcurve.push((n, v));
                self.velcurve.sort_by_key(|p| p.0);
                Applied::Taken
            }
            "amp_random" => set!(self.amp_random, f()),
            "rt_decay" => set!(self.rt_decay, f()),
            "xfin_lovel" => set!(self.xfin_vel.0, key7(i())),
            "xfin_hivel" => set!(self.xfin_vel.1, key7(i())),
            "xfout_lovel" => set!(self.xfout_vel.0, key7(i())),
            "xfout_hivel" => set!(self.xfout_vel.1, key7(i())),
            "xfin_lokey" => set!(self.xfin_key.0, k()),
            "xfin_hikey" => set!(self.xfin_key.1, k()),
            "xfout_lokey" => set!(self.xfout_key.0, k()),
            "xfout_hikey" => set!(self.xfout_key.1, k()),
            "xf_velcurve" | "xf_keycurve" | "xf_cccurve" => {
                let curve = match value.trim() {
                    "power" => Crossfade::Power,
                    "gain" => Crossfade::Gain,
                    _ => return bad(),
                };
                match pattern.as_str() {
                    "xf_velcurve" => self.xf_velcurve = curve,
                    "xf_keycurve" => self.xf_keycurve = curve,
                    _ => self.xf_cccurve = curve,
                }
                Applied::Taken
            }
            "xfin_locc&" | "xfin_hicc&" | "xfout_locc&" | "xfout_hicc&" => {
                let (Some(cc), Some(v)) = (cc_number(numbers.first()), f()) else {
                    return bad();
                };
                let fade = self.cc_fade(cc);
                match pattern.as_str() {
                    "xfin_locc&" => fade.fade_in.0 = v,
                    "xfin_hicc&" => fade.fade_in.1 = v,
                    "xfout_locc&" => fade.fade_out.0 = v,
                    _ => fade.fade_out.1 = v,
                }
                Applied::Taken
            }
            // Filter.
            "fil_type" | "filtype" | "fil&_type" => {
                if pattern == "fil&_type" && numbers.first() != Some(&1) {
                    return Applied::Unknown(pattern.clone());
                }
                set!(self.filter, FilterKind::parse(value).map(Some))
            }
            "cutoff" => set!(self.cutoff, f().map(Some)),
            "cutoff&" if numbers.first() == Some(&1) => set!(self.cutoff, f().map(Some)),
            "resonance" => set!(self.resonance, f()),
            "resonance&" if numbers.first() == Some(&1) => set!(self.resonance, f()),
            "fil_keytrack" | "fil&_keytrack" => set!(self.fil_keytrack, f()),
            "fil_keycenter" | "fil&_keycenter" => set!(self.fil_keycenter, k()),
            "fil_veltrack" | "fil&_veltrack" => set!(self.fil_veltrack, f()),
            "fil_random" | "fil&_random" => set!(self.fil_random, f()),
            // LFOs, SFZ 1's.
            "pitchlfo_depth" => set!(self.pitchlfo.depth, f()),
            "pitchlfo_freq" => set!(self.pitchlfo.freq, f()),
            "pitchlfo_delay" => set!(self.pitchlfo.delay, f()),
            "pitchlfo_fade" => set!(self.pitchlfo.fade, f()),
            "amplfo_depth" => set!(self.amplfo.depth, f()),
            "amplfo_freq" => set!(self.amplfo.freq, f()),
            "amplfo_delay" => set!(self.amplfo.delay, f()),
            "amplfo_fade" => set!(self.amplfo.fade, f()),
            "fillfo_depth" => set!(self.fillfo.depth, f()),
            "fillfo_freq" => set!(self.fillfo.freq, f()),
            "fillfo_delay" => set!(self.fillfo.delay, f()),
            "fillfo_fade" => set!(self.fillfo.fade, f()),
            p if p.starts_with("lfo&_") && p.matches('&').count() == 1 => {
                let Some(index) = numbers.first().and_then(|&n| u8::try_from(n).ok()) else {
                    return bad();
                };
                let rest = &p["lfo&_".len()..];
                if rest == "wave" {
                    return match i().and_then(Wave::of) {
                        Some(w) => {
                            self.lfo(index).wave = w;
                            Applied::Taken
                        }
                        None => bad(),
                    };
                }
                let Some(v) = f() else {
                    return bad();
                };
                let lfo = self.lfo(index);
                match rest {
                    "freq" => lfo.freq = v,
                    "delay" => lfo.delay = v,
                    "fade" => lfo.fade = v,
                    "phase" => lfo.phase = v,
                    "pitch" => lfo.pitch = v,
                    "volume" => lfo.volume = v,
                    "amplitude" => lfo.amplitude = v,
                    "pan" => lfo.pan = v,
                    "cutoff" | "cutoff&" => lfo.cutoff = v,
                    _ => return Applied::Unknown(pattern.clone()),
                }
                Applied::Taken
            }
            p if ignored(p) => Applied::Ignored,
            p => {
                // Envelopes.
                for (prefix, kind) in [
                    ("ampeg_", EgKind::Amp),
                    ("fileg_", EgKind::Filter),
                    ("pitcheg_", EgKind::Pitch),
                ] {
                    if let Some(rest) = p.strip_prefix(prefix)
                        && let Some(applied) = self.envelope(kind, rest, value)
                    {
                        return applied;
                    }
                }
                // Controllers.
                if let Some((target, part)) = controller(p, &numbers) {
                    let Some(cc) = cc_number(numbers.last()) else {
                        return bad();
                    };
                    return match part {
                        CcPart::Ignored => Applied::Ignored,
                        CcPart::Depth => match f() {
                            Some(v) => {
                                self.modulation(target, cc).depth = v;
                                Applied::Taken
                            }
                            None => bad(),
                        },
                        CcPart::Curve => match i().and_then(|v| u16::try_from(v).ok()) {
                            Some(v) => {
                                self.modulation(target, cc).curve = v;
                                Applied::Taken
                            }
                            None => bad(),
                        },
                    };
                }
                Applied::Unknown(pattern.clone())
            }
        }
    }

    /// An envelope opcode, `rest` after its `ampeg_` (or `fileg_`,
    /// `pitcheg_`); `None` when it isn't one.
    fn envelope(&mut self, kind: EgKind, rest: &str, value: &str) -> Option<Applied> {
        let eg = match kind {
            EgKind::Amp => &mut self.ampeg,
            EgKind::Filter => &mut self.fileg,
            EgKind::Pitch => &mut self.pitcheg,
        };
        let field = match rest {
            "delay" => &mut eg.delay,
            "start" => &mut eg.start,
            "attack" => &mut eg.attack,
            "hold" => &mut eg.hold,
            "decay" => &mut eg.decay,
            "sustain" => &mut eg.sustain,
            "release" => &mut eg.release,
            "depth" if kind != EgKind::Amp => &mut eg.depth,
            "vel2delay" => &mut eg.vel2_delay,
            "vel2attack" => &mut eg.vel2_attack,
            "vel2hold" => &mut eg.vel2_hold,
            "vel2decay" => &mut eg.vel2_decay,
            "vel2sustain" => &mut eg.vel2_sustain,
            "vel2release" => &mut eg.vel2_release,
            "vel2depth" if kind != EgKind::Amp => &mut eg.vel2_depth,
            "attack_shape" => &mut eg.attack_shape,
            "decay_shape" => &mut eg.decay_shape,
            "release_shape" => &mut eg.release_shape,
            _ => return None,
        };
        Some(match number(value) {
            Some(v) => {
                *field = v;
                Applied::Taken
            }
            None => Applied::Bad(format!("{rest}={value}")),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn region(opcodes: &[(&str, &str)]) -> (Region, Context, Vec<Applied>) {
        let mut ctx = Context {
            base: PathBuf::from("/k/Programs"),
            ..Context::default()
        };
        let mut r = Region::default();
        let applied = opcodes
            .iter()
            .map(|(n, v)| r.apply(n, v, &mut ctx))
            .collect();
        (r, ctx, applied)
    }

    #[test]
    fn note_names_and_numbers() {
        assert_eq!(key("60"), Some(60));
        assert_eq!(key("c4"), Some(60));
        assert_eq!(key("C#4"), Some(61));
        assert_eq!(key("db4"), Some(61));
        assert_eq!(key("a-1"), Some(9));
        assert_eq!(key("e3"), Some(52));
        assert_eq!(key("h4"), None);
        assert_eq!(number("0.3s"), Some(0.3));
        assert_eq!(number("-12"), Some(-12.0));
        assert_eq!(number("-1.6."), Some(-1.6));
        assert_eq!(number("x"), None);
    }

    #[test]
    fn opcode_names_take_their_numbers_out() {
        assert_eq!(
            shape("lfo02_pitch_oncc113"),
            ("lfo&_pitch_oncc&".to_owned(), vec![2, 113])
        );
        assert_eq!(
            shape("amp_velcurve_42"),
            ("amp_velcurve_&".to_owned(), vec![42])
        );
        assert_eq!(shape("sample"), ("sample".to_owned(), vec![]));
    }

    #[test]
    fn samples_are_listed_once_from_the_top_files_folder() {
        let (r, ctx, applied) = region(&[
            ("sample", "../Samples/ord/a.wav"),
            ("sample", "..\\Samples\\ord\\a.wav"),
        ]);
        assert!(applied.iter().all(|a| *a == Applied::Taken));
        assert_eq!(r.source, Some(Source::File(0)));
        assert_eq!(ctx.samples, [PathBuf::from("/k/Samples/ord/a.wav")]);
        let mut ctx = Context {
            base: PathBuf::from("/k"),
            default_path: "Kit/".into(),
            ..Context::default()
        };
        let mut r = Region::default();
        r.apply("sample", "snare.flac", &mut ctx);
        r.apply("sample", "*sine", &mut ctx);
        assert_eq!(r.source, Some(Source::Sine));
        assert_eq!(ctx.samples, [PathBuf::from("/k/Kit/snare.flac")]);
    }

    #[test]
    fn keys_velocities_and_switches() {
        let (r, _, applied) = region(&[
            ("key", "e3"),
            ("lovel", "43"),
            ("hivel", "200"),
            ("sw_lokey", "36"),
            ("sw_hikey", "38"),
            ("sw_last", "37"),
            ("sw_default", "c2"),
            ("trigger", "release"),
            ("seq_length", "4"),
            ("seq_position", "3"),
            ("lorand", "0.25"),
            ("hirand", "0.5"),
            ("group", "1"),
            ("off_by", "2"),
            ("off_mode", "normal"),
        ]);
        assert!(applied.iter().all(|a| *a == Applied::Taken), "{applied:?}");
        assert_eq!(
            (r.lokey, r.hikey, r.keycenter),
            (52, 52, KeyCenter::Key(52))
        );
        assert_eq!((r.lovel, r.hivel), (43, 127));
        assert_eq!(
            (r.sw_lokey, r.sw_hikey, r.sw_last),
            (Some(36), Some(38), Some((37, 37)))
        );
        assert_eq!(r.sw_default, Some(36));
        assert_eq!(r.trigger, Trigger::Release);
        assert_eq!((r.seq_length, r.seq_position), (4, 3));
        assert_eq!((r.lorand, r.hirand), (0.25, 0.5));
        assert_eq!(
            (r.group, r.off_by, r.off_mode),
            (1, Some(2), OffMode::Normal)
        );
    }

    #[test]
    fn controllers_are_kept_as_written() {
        let (r, _, applied) = region(&[
            ("amplitude_oncc7", "100"),
            ("amplitude_curvecc7", "4"),
            ("ampeg_attackcc1", "0.5"),
            ("ampeg_release_oncc72", "2"),
            ("lfo02_pitch_oncc113", "104"),
            ("pan_smoothcc10", "10"),
            ("tune_cc128", "200"),
            ("locc64", "64"),
            ("hihdcc1", "0.5"),
            ("xfin_locc1", "10"),
            ("xfin_hicc1", "40"),
            ("on_locc64", "64"),
            ("group_volume", "-6"),
        ]);
        assert_eq!(applied[5], Applied::Ignored);
        assert!(
            applied
                .iter()
                .enumerate()
                .all(|(i, a)| i == 5 || *a == Applied::Taken),
            "{applied:?}"
        );
        assert_eq!(
            r.mods,
            [
                Mod {
                    target: Target::Amplitude,
                    cc: 7,
                    depth: 100.0,
                    curve: 4
                },
                Mod {
                    target: Target::Eg(EgKind::Amp, EgParam::Attack),
                    cc: 1,
                    depth: 0.5,
                    curve: 0
                },
                Mod {
                    target: Target::Eg(EgKind::Amp, EgParam::Release),
                    cc: 72,
                    depth: 2.0,
                    curve: 0
                },
                Mod {
                    target: Target::Lfo(2, LfoParam::Pitch),
                    cc: 113,
                    depth: 104.0,
                    curve: 0
                },
                Mod {
                    target: Target::Tune,
                    cc: 128,
                    depth: 200.0,
                    curve: 0
                },
            ]
        );
        assert_eq!(r.cc_ranges, [(64, 64.0, 127.0), (1, 0.0, 63.5)]);
        assert!(r.cc_triggered);
        assert_eq!(r.group_volume, -6.0);
        assert_eq!(
            r.cc_fades,
            [CcFade {
                cc: 1,
                fade_in: (10.0, 40.0),
                fade_out: (127.0, 127.0)
            }]
        );
    }

    #[test]
    fn envelopes_lfos_and_filters() {
        let (r, _, applied) = region(&[
            ("ampeg_attack", "0.01"),
            ("ampeg_release", "0.25"),
            ("ampeg_sustain", "0"),
            ("ampeg_decay_shape", "-2"),
            ("fileg_depth", "1200"),
            ("lfo01_freq", "5.5"),
            ("lfo01_wave", "1"),
            ("lfo01_pitch", "30"),
            ("pitchlfo_depth", "20"),
            ("fil_type", "hpf_2p"),
            ("cutoff", "800"),
            ("resonance", "3"),
            ("amp_velcurve_42", "1"),
            ("amp_velcurve_1", "0.2"),
        ]);
        assert!(applied.iter().all(|a| *a == Applied::Taken), "{applied:?}");
        assert_eq!(
            (r.ampeg.attack, r.ampeg.release, r.ampeg.sustain),
            (0.01, 0.25, 0.0)
        );
        assert_eq!(r.ampeg.decay_shape, -2.0);
        assert_eq!(r.fileg.depth, 1200.0);
        assert_eq!(r.lfos.len(), 1);
        assert_eq!(
            (r.lfos[0].freq, r.lfos[0].wave, r.lfos[0].pitch),
            (5.5, Wave::Sine, 30.0)
        );
        assert_eq!(r.pitchlfo.depth, 20.0);
        assert_eq!(
            (r.filter, r.cutoff, r.resonance),
            (Some(FilterKind::HighPass2), Some(800.0), 3.0)
        );
        assert_eq!(r.velcurve, [(1, 0.2), (42, 1.0)]);
    }

    #[test]
    fn unknown_and_bad_opcodes_are_said() {
        let (r, _, applied) = region(&[
            ("eq1_gain", "3"),
            ("lokey", "zz"),
            ("label_cc27", "Release vol"),
            ("ampeg_depth", "3"),
        ]);
        assert_eq!(applied[0], Applied::Unknown("eq&_gain".into()));
        assert_eq!(applied[1], Applied::Bad("lokey=zz".into()));
        assert_eq!(applied[2], Applied::Ignored);
        assert_eq!(applied[3], Applied::Unknown("ampeg_depth".into()));
        assert_eq!(r.lokey, 0);
    }

    #[test]
    fn a_key_offset_moves_every_key() {
        let mut ctx = Context {
            key_offset: -12,
            ..Context::default()
        };
        let mut r = Region::default();
        r.apply("key", "60", &mut ctx);
        r.apply("sw_last", "36", &mut ctx);
        assert_eq!((r.lokey, r.sw_last), (48, Some((24, 24))));
    }
}
