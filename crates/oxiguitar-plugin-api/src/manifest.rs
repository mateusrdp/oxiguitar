// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The manifest: what a plugin is and adds, read without loading its code.
//!
//! A manifest is a JSON object, written by the plugin's author as
//! `manifest.json` beside its `Cargo.toml`. The SDK's `export_plugin!`
//! puts the file's bytes in the plugin's manifest section
//! ([`crate::MANIFEST_SECTION`]), where the host reads them with the
//! `object` crate. Keys a host doesn't know are ignored, so a newer plugin
//! still loads in an older host; the SDK's checks reject unknown keys at
//! build time, so typing mistakes don't go unnoticed.
//!
//! ```json
//! {
//!   "interface": "1.0",
//!   "id": "oxiguitar.gp5",
//!   "name": "Guitar Pro 5",
//!   "version": "0.2.0",
//!   "description": "Reads Guitar Pro 5 files (.gp5).",
//!   "authors": ["The OxiGuitar contributors"],
//!   "license": "GPL-3.0-or-later",
//!   "formats": [{
//!     "id": "gp5", "name": "Guitar Pro 5", "extensions": ["gp5"],
//!     "read": true,
//!     "signatures": [{ "at": 1, "text": "FICHIER GUITAR PRO v5" }]
//!   }]
//! }
//! ```
//!
//! Interface 1.1 added `tabs` and `events`, 1.2 `tools`, 1.3
//! `settings_window`, `track_sections` and `folders` (and brought
//! `instruments` to life). Later minor
//! versions add keys for exporters and toolbar sections when plugins need
//! them (docs/design/plugins.md, "What plugins contribute").

use std::cmp::Ordering;
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::class::InstrumentClass;
use crate::event::EventKind;
use crate::signature::{Signature, Sniff};
use crate::tab::TabSpec;

/// A plugin's manifest.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
    /// The interface version the plugin was built for.
    pub interface: InterfaceVersion,
    /// A unique id: lowercase letters and digits in parts joined by `.`
    /// or `-`, such as `oxiguitar.gp5`. Our plugins start with
    /// `oxiguitar.`; others should start with their author's own name.
    pub id: String,
    /// The name shown to the user.
    pub name: String,
    /// The plugin's version, `MAJOR.MINOR.PATCH` with an optional
    /// `-PRERELEASE`.
    pub version: Version,
    /// One or two sentences for the plugin manager.
    #[serde(default)]
    pub description: String,
    /// Who wrote it.
    #[serde(default)]
    pub authors: Vec<String>,
    /// Its licence, as an SPDX expression.
    #[serde(default)]
    pub license: String,
    /// Where it comes from.
    #[serde(default)]
    pub homepage: String,
    /// Where the plugin can run; it is never loaded elsewhere.
    #[serde(default)]
    pub conditions: Conditions,
    /// The service interfaces it calls through the host.
    #[serde(default)]
    pub requires: Vec<String>,
    /// The file formats it reads or writes.
    #[serde(default)]
    pub formats: Vec<FormatSpec>,
    /// The instruments it offers the player. The player uses them from
    /// interface 1.3 on, and ignores instruments declared by a plugin built
    /// for an older interface.
    #[serde(default)]
    pub instruments: Vec<InstrumentSpec>,
    /// The services it implements.
    #[serde(default)]
    pub services: Vec<ServiceSpec>,
    /// The tabs it adds to the window's areas. Since interface 1.1.
    #[serde(default)]
    pub tabs: Vec<TabSpec>,
    /// The kinds of event it wants pushed to it. Since interface 1.1.
    #[serde(default)]
    pub events: Vec<EventKind>,
    /// The tools it adds to the Tools menu. Since interface 1.2.
    #[serde(default)]
    pub tools: Vec<ToolSpec>,
    /// Whether it has a settings window, opened from the plugin manager's
    /// Settings… (`Request::SettingsWindow`). Since interface 1.3.
    #[serde(default)]
    pub settings_window: bool,
    /// The sections it adds to the track dialog. Since interface 1.3.
    #[serde(default)]
    pub track_sections: Vec<TrackSectionSpec>,
    /// The folders it shares with the user and other plugins, which the
    /// host creates at start and hands over (`HostRequest::Folders`).
    /// Since interface 1.3.
    #[serde(default)]
    pub folders: Vec<SharedFolder>,
    /// The effects it offers a track's sound, after its instrument. Since
    /// interface 1.3.
    #[serde(default)]
    pub effects: Vec<InstrumentSpec>,
}

/// A section of the track dialog, as the manifest declares it: rows of
/// widgets under a heading, for the tracks of the classes it names. What the
/// user sets in it is kept per track, saved beside the score
/// (`song.gp.oxg`), and handed to the plugin with every request about that
/// track. Since interface 1.3.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrackSectionSpec {
    /// The id requests name it by, unique in the plugin.
    pub id: String,
    /// The section's heading: a word or two.
    pub title: String,
    /// The classes of track it shows for; absent means every class.
    #[serde(default)]
    pub plays: Option<Vec<InstrumentClass>>,
    /// Classes taken out of `plays`.
    #[serde(default)]
    pub does_not_play: Vec<InstrumentClass>,
}

impl TrackSectionSpec {
    /// Whether this section shows for tracks of class `class`.
    pub fn shows_for(&self, class: InstrumentClass) -> bool {
        self.plays.as_ref().is_none_or(|p| p.contains(&class))
            && !self.does_not_play.contains(&class)
    }
}

/// A folder in the user's OxiGuitar folder that the user fills and plugins
/// read: sound sets, amp models. Since interface 1.3.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SharedFolder {
    /// `soundfonts`: sound sets (.sf2, .sf3, .dls, SFZ instruments' folders).
    Soundfonts,
    /// `nam`: amp models (.nam) and cabinet impulse responses (.wav).
    Nam,
}

impl SharedFolder {
    /// Every shared folder.
    pub const ALL: [SharedFolder; 2] = [SharedFolder::Soundfonts, SharedFolder::Nam];

    /// The folder's name in the user's OxiGuitar folder.
    pub fn name(self) -> &'static str {
        match self {
            SharedFolder::Soundfonts => "soundfonts",
            SharedFolder::Nam => "nam",
        }
    }
}

/// A tool, as the manifest declares it: an entry of the Tools menu that
/// the core runs by asking the plugin (`Request::Tool`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolSpec {
    /// The id requests name it by, unique in the plugin.
    pub id: String,
    /// The menu entry, in title case: "Transpose by Octave".
    pub name: String,
    /// What it does, in a sentence: shown with the tool where there is
    /// room (`oxiguitar-cli tools`).
    #[serde(default)]
    pub description: String,
}

impl Manifest {
    /// Reads a manifest from a manifest section's bytes: JSON text,
    /// followed by any number of zero bytes (padding some linkers add).
    /// Checks it too ([`Manifest::problems`]); the first problem is the
    /// error.
    pub fn parse(bytes: &[u8]) -> Result<Manifest, String> {
        if bytes.len() > crate::MAX_MANIFEST_LEN {
            return Err(format!(
                "the manifest is {} bytes, more than the {} a manifest may have",
                bytes.len(),
                crate::MAX_MANIFEST_LEN
            ));
        }
        let end = bytes.iter().rposition(|&b| b != 0).map_or(0, |i| i + 1);
        let text = std::str::from_utf8(&bytes[..end])
            .map_err(|e| format!("the manifest is not UTF-8 text ({e})"))?;
        if text.trim().is_empty() {
            return Err("the manifest is empty".into());
        }
        let manifest: Manifest =
            serde_json::from_str(text).map_err(|e| format!("the manifest is not valid ({e})"))?;
        match manifest.problems().into_iter().next() {
            Some(p) => Err(p),
            None => Ok(manifest),
        }
    }

    /// Everything wrong with the manifest beyond its JSON shape.
    pub fn problems(&self) -> Vec<String> {
        let mut out = Vec::new();
        if !is_id(&self.id) {
            out.push(format!(
                "the id {:?} is not lowercase letters and digits joined by . or -",
                self.id
            ));
        }
        if self.name.trim().is_empty() {
            out.push("the name is empty".into());
        }
        let mut formats: Vec<&str> = Vec::new();
        for f in &self.formats {
            let at = format!("format {:?}", f.id);
            if !is_id(&f.id) {
                out.push(format!("{at}: the id is not a valid id"));
            }
            if formats.contains(&f.id.as_str()) {
                out.push(format!("{at}: declared twice"));
            }
            formats.push(&f.id);
            if f.name.trim().is_empty() {
                out.push(format!("{at}: the name is empty"));
            }
            if !f.read && !f.write {
                out.push(format!("{at}: neither reads nor writes"));
            }
            if f.read && f.signatures.is_empty() {
                out.push(format!(
                    "{at}: a format that reads needs a signature to recognise its files"
                ));
            }
            for s in &f.signatures {
                if let Some(p) = s.problem() {
                    out.push(format!("{at}: {p}"));
                }
            }
            for e in &f.extensions {
                if e.is_empty() || e.starts_with('.') || e.to_lowercase() != *e {
                    out.push(format!(
                        "{at}: the extension {e:?} must be lowercase and without a dot"
                    ));
                }
            }
        }
        let mut instruments: Vec<&str> = Vec::new();
        for i in &self.instruments {
            if !is_id(&i.id) || instruments.contains(&i.id.as_str()) {
                out.push(format!("instrument {:?}: not a valid or unique id", i.id));
            }
            instruments.push(&i.id);
            if InstrumentClass::ALL.iter().all(|&c| !i.plays(c)) {
                out.push(format!("instrument {:?}: plays nothing", i.id));
            }
        }
        let mut services: Vec<(&str, &str)> = Vec::new();
        for s in &self.services {
            let key = (s.interface.as_str(), s.id.as_str());
            if !is_id(&s.interface) || !is_id(&s.id) || services.contains(&key) {
                out.push(format!(
                    "service {:?} of {:?}: not a valid or unique interface and id",
                    s.id, s.interface
                ));
            }
            services.push(key);
        }
        for r in &self.requires {
            if !is_id(r) {
                out.push(format!("requires {r:?}: not a valid interface name"));
            }
        }
        let mut tabs: Vec<&str> = Vec::new();
        for t in &self.tabs {
            if !is_id(&t.id) || tabs.contains(&t.id.as_str()) {
                out.push(format!("tab {:?}: not a valid or unique id", t.id));
            }
            tabs.push(&t.id);
            if t.title.trim().is_empty() {
                out.push(format!("tab {:?}: the title is empty", t.id));
            }
        }
        let mut tools: Vec<&str> = Vec::new();
        for t in &self.tools {
            if !is_id(&t.id) || tools.contains(&t.id.as_str()) {
                out.push(format!("tool {:?}: not a valid or unique id", t.id));
            }
            tools.push(&t.id);
            if t.name.trim().is_empty() {
                out.push(format!("tool {:?}: the name is empty", t.id));
            }
        }
        let mut effects: Vec<&str> = Vec::new();
        for e in &self.effects {
            if !is_id(&e.id) || effects.contains(&e.id.as_str()) {
                out.push(format!("effect {:?}: not a valid or unique id", e.id));
            }
            effects.push(&e.id);
            if InstrumentClass::ALL.iter().all(|&c| !e.plays(c)) {
                out.push(format!("effect {:?}: plays nothing", e.id));
            }
        }
        let mut sections: Vec<&str> = Vec::new();
        for s in &self.track_sections {
            if !is_id(&s.id) || sections.contains(&s.id.as_str()) {
                out.push(format!(
                    "track section {:?}: not a valid or unique id",
                    s.id
                ));
            }
            sections.push(&s.id);
            if s.title.trim().is_empty() {
                out.push(format!("track section {:?}: the title is empty", s.id));
            }
            if InstrumentClass::ALL.iter().all(|&c| !s.shows_for(c)) {
                out.push(format!("track section {:?}: shows for no track", s.id));
            }
        }
        for (i, f) in self.folders.iter().enumerate() {
            if self.folders[..i].contains(f) {
                out.push(format!("folder {:?}: listed twice", f.name()));
            }
        }
        let since_1_3 = InterfaceVersion { major: 1, minor: 3 };
        if self.interface < since_1_3 {
            for (what, used) in [
                ("a settings window needs", self.settings_window),
                ("track sections need", !self.track_sections.is_empty()),
                ("shared folders need", !self.folders.is_empty()),
                ("effects need", !self.effects.is_empty()),
                // A host before 1.3 would ignore the condition and load
                // code its processor can't run: such a host refuses a 1.3
                // manifest, so the key may appear only in one.
                (
                    "processor features (conditions \"cpu\") need",
                    !self.conditions.cpu.is_empty()
                        || self.services.iter().any(|s| !s.conditions.cpu.is_empty()),
                ),
            ] {
                if used {
                    out.push(format!(
                        "{what} interface {since_1_3}, and the manifest asks for {}",
                        self.interface
                    ));
                }
            }
        }
        let since_1_2 = InterfaceVersion { major: 1, minor: 2 };
        if self.interface < since_1_2 && !self.tools.is_empty() {
            out.push(format!(
                "tools need interface {since_1_2}, and the manifest asks for {}",
                self.interface
            ));
        }
        let since_1_1 = InterfaceVersion { major: 1, minor: 1 };
        if self.interface < since_1_1 {
            if !self.tabs.is_empty() {
                out.push(format!(
                    "tabs need interface {since_1_1}, and the manifest asks for {}",
                    self.interface
                ));
            }
            if !self.events.is_empty() {
                out.push(format!(
                    "events need interface {since_1_1}, and the manifest asks for {}",
                    self.interface
                ));
            }
        }
        out
    }

    /// Whether the plugin knows interface 1.1 (tabs, events, the score)
    /// and later minor versions up to `minor`.
    pub fn knows_minor(&self, minor: u32) -> bool {
        self.interface.minor >= minor
    }

    /// Whether the plugin wants events of `kind`.
    pub fn wants(&self, kind: EventKind) -> bool {
        self.knows_minor(1) && self.events.contains(&kind)
    }

    /// The first format this plugin reads whose signatures match `file`.
    pub fn format_for(&self, file: &Sniff<'_>) -> Option<&FormatSpec> {
        self.formats
            .iter()
            .filter(|f| f.read)
            .find(|f| f.signatures.iter().any(|s| s.matches(file)))
    }
}

/// Whether `s` is a valid id: 1–64 characters, lowercase ASCII letters and
/// digits in parts joined by single `.` or `-`.
pub fn is_id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s.split(['.', '-']).all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        })
}

/// A file format a plugin reads or writes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FormatSpec {
    /// The id requests name it by, unique in the plugin, such as `gp5`.
    pub id: String,
    /// The name shown in dialogs and messages, such as "Guitar Pro 5".
    pub name: String,
    /// File extensions for open and save dialogs, lowercase, without the
    /// dot, the usual one first.
    #[serde(default)]
    pub extensions: Vec<String>,
    /// Media types, such as `application/x-gtp`.
    #[serde(default)]
    pub media_types: Vec<String>,
    /// Whether the plugin reads this format.
    #[serde(default)]
    pub read: bool,
    /// Whether the plugin writes this format.
    #[serde(default)]
    pub write: bool,
    /// How to recognise a file of this format; a file is of this format
    /// when any signature matches.
    #[serde(default)]
    pub signatures: Vec<Signature>,
}

/// An instrument a plugin offers the player.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentSpec {
    /// The id, unique in the plugin.
    pub id: String,
    /// The name shown in the Output menu and the track's instrument choice.
    pub name: String,
    /// The classes it plays; absent means every class.
    #[serde(default)]
    pub plays: Option<Vec<InstrumentClass>>,
    /// Classes taken out of `plays`.
    #[serde(default)]
    pub does_not_play: Vec<InstrumentClass>,
}

impl InstrumentSpec {
    /// Whether this instrument plays tracks of class `class`.
    pub fn plays(&self, class: InstrumentClass) -> bool {
        self.plays.as_ref().is_none_or(|p| p.contains(&class))
            && !self.does_not_play.contains(&class)
    }
}

/// A service: one implementation of an interface, chosen per platform and
/// environment.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceSpec {
    /// The interface it implements, such as `audio-device`.
    pub interface: String,
    /// The id, unique for the interface in the plugin, such as `pipewire`.
    pub id: String,
    /// The name shown to the user.
    #[serde(default)]
    pub name: String,
    /// Higher wins when several services of one interface are available
    /// and settings.json pins none.
    #[serde(default)]
    pub priority: i32,
    /// Where this service can run, beyond the plugin's own conditions.
    #[serde(default)]
    pub conditions: Conditions,
}

/// Where something can run. Every list given must contain the running
/// system's value; an empty list allows any.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Conditions {
    /// Operating systems: `linux`, `macos`, `windows`, `android`, `ios`.
    #[serde(default)]
    pub os: Vec<String>,
    /// Processors: `x86_64`, `aarch64`.
    #[serde(default)]
    pub arch: Vec<String>,
    /// Display systems: `wayland`, `x11`, `windows`, `macos` (the core's
    /// build decides which; docs/design/plugins.md has the two Linux
    /// builds).
    #[serde(default)]
    pub display: Vec<String>,
    /// Processor features the code needs, all of them, as Rust names them
    /// (`avx2`, `fma`, `avx512f`, `neon`…): a build for x86-64-v3 lists
    /// `avx2` and `fma`, so a processor without them never loads it and
    /// another file of the same plugin, built without them, is used. A
    /// name this OxiGuitar doesn't know counts as missing. When two usable
    /// files hold the same plugin at the same version, the one needing
    /// more features is used. Since interface 1.3.
    #[serde(default)]
    pub cpu: Vec<String>,
}

impl Conditions {
    /// Why these conditions don't hold in `env`, or `None` if they do.
    pub fn unmet(&self, env: &Environment) -> Option<String> {
        let check = |what: &str, allowed: &[String], here: &str| {
            (!allowed.is_empty() && !allowed.iter().any(|a| a == here)).then(|| {
                format!(
                    "it runs only with {what} {}, and this is {here}",
                    allowed.join(" or ")
                )
            })
        };
        let missing: Vec<&str> = self
            .cpu
            .iter()
            .filter(|f| !env.cpu.contains(f))
            .map(String::as_str)
            .collect();
        check("the system", &self.os, &env.os)
            .or_else(|| check("the processor", &self.arch, &env.arch))
            .or_else(|| check("the display system", &self.display, &env.display))
            .or_else(|| {
                (!missing.is_empty()).then(|| {
                    format!(
                        "it needs the processor feature{} {}, which this processor doesn't have",
                        if missing.len() == 1 { "" } else { "s" },
                        missing.join(", ")
                    )
                })
            })
    }
}

/// The processor features this process can use, of those manifests may
/// name in [`Conditions::cpu`], as Rust names them.
pub fn cpu_features() -> Vec<String> {
    #[allow(unused_mut)]
    let mut out: Vec<&str> = Vec::new();
    macro_rules! detect {
        ($check:ident, $($f:tt),*) => {
            $( if std::arch::$check!($f) { out.push($f); } )*
        };
    }
    #[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
    detect!(
        is_x86_feature_detected,
        "sse3",
        "ssse3",
        "sse4.1",
        "sse4.2",
        "popcnt",
        "avx",
        "avx2",
        "fma",
        "f16c",
        "bmi1",
        "bmi2",
        "lzcnt",
        "movbe",
        "avx512f",
        "avx512bw",
        "avx512cd",
        "avx512dq",
        "avx512vl"
    );
    #[cfg(target_arch = "aarch64")]
    detect!(
        is_aarch64_feature_detected,
        "neon",
        "aes",
        "sha2",
        "crc",
        "lse",
        "rdm",
        "dotprod",
        "fp16",
        "fhm",
        "sve",
        "sve2",
        "i8mm",
        "bf16"
    );
    out.into_iter().map(str::to_owned).collect()
}

/// The system the host runs on, as [`Conditions`] name it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Environment {
    /// The operating system.
    pub os: String,
    /// The processor.
    pub arch: String,
    /// The display system the core was built for.
    pub display: String,
    /// The processor's features ([`cpu_features`]).
    pub cpu: Vec<String>,
}

impl Environment {
    /// This process's system and processor, with the core's display system.
    pub fn current(display: &str) -> Environment {
        Environment {
            os: std::env::consts::OS.into(),
            arch: std::env::consts::ARCH.into(),
            display: display.into(),
            cpu: cpu_features(),
        }
    }
}

/// An interface version, written `"MAJOR.MINOR"` in manifests.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InterfaceVersion {
    /// Incompatible versions.
    pub major: u32,
    /// Additions within a major version.
    pub minor: u32,
}

impl fmt::Display for InterfaceVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

impl FromStr for InterfaceVersion {
    type Err = String;

    fn from_str(s: &str) -> Result<InterfaceVersion, String> {
        let bad = || format!("{s:?} is not an interface version such as \"1.0\"");
        let (major, minor) = s.split_once('.').ok_or_else(bad)?;
        Ok(InterfaceVersion {
            major: number(major).ok_or_else(bad)?,
            minor: number(minor).ok_or_else(bad)?,
        })
    }
}

/// A plugin's version: `MAJOR.MINOR.PATCH`, optionally followed by
/// `-PRERELEASE`. A pre-release comes before its release; two
/// pre-releases compare by their text.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Version {
    /// Major version.
    pub major: u64,
    /// Minor version.
    pub minor: u64,
    /// Patch version.
    pub patch: u64,
    /// The pre-release part, empty for a release.
    pub pre: String,
}

impl Ord for Version {
    fn cmp(&self, other: &Version) -> Ordering {
        (self.major, self.minor, self.patch)
            .cmp(&(other.major, other.minor, other.patch))
            .then_with(|| match (self.pre.is_empty(), other.pre.is_empty()) {
                (true, true) => Ordering::Equal,
                (true, false) => Ordering::Greater,
                (false, true) => Ordering::Less,
                (false, false) => self.pre.cmp(&other.pre),
            })
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Version) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)?;
        if !self.pre.is_empty() {
            write!(f, "-{}", self.pre)?;
        }
        Ok(())
    }
}

impl FromStr for Version {
    type Err = String;

    fn from_str(s: &str) -> Result<Version, String> {
        let bad = || format!("{s:?} is not a version such as \"1.2.3\"");
        let (numbers, pre) = s.split_once('-').unwrap_or((s, ""));
        let mut parts = numbers.split('.');
        let mut next = || parts.next().and_then(number).ok_or_else(bad);
        let version = Version {
            major: next()?,
            minor: next()?,
            patch: next()?,
            pre: pre.to_owned(),
        };
        if parts.next().is_some() || (s.contains('-') && pre.is_empty()) {
            return Err(bad());
        }
        Ok(version)
    }
}

/// A decimal number with no sign, no leading zeros (other than "0") and
/// no other characters.
fn number<T: FromStr>(s: &str) -> Option<T> {
    let digits = !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    (digits && (s == "0" || !s.starts_with('0')))
        .then(|| s.parse().ok())
        .flatten()
}

macro_rules! as_string {
    ($t:ty) => {
        impl Serialize for $t {
            fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                s.collect_str(self)
            }
        }

        impl<'de> Deserialize<'de> for $t {
            fn deserialize<D: Deserializer<'de>>(d: D) -> Result<$t, D::Error> {
                let s = String::deserialize(d)?;
                s.parse().map_err(serde::de::Error::custom)
            }
        }
    };
}

as_string!(InterfaceVersion);
as_string!(Version);

#[cfg(test)]
mod tests {
    use super::*;

    const GP5: &str = r#"{
        "interface": "1.0",
        "id": "oxiguitar.gp5",
        "name": "Guitar Pro 5",
        "version": "0.2.0",
        "formats": [{
            "id": "gp5", "name": "Guitar Pro 5", "extensions": ["gp5"],
            "read": true,
            "signatures": [{ "at": 1, "text": "FICHIER GUITAR PRO v5" }]
        }],
        "a_key_from_a_later_version": { "ignored": true }
    }"#;

    #[test]
    fn a_manifest_reads_with_padding_and_unknown_keys() {
        let mut bytes = GP5.as_bytes().to_vec();
        bytes.extend([0; 300]);
        let m = Manifest::parse(&bytes).unwrap();
        assert_eq!(m.id, "oxiguitar.gp5");
        assert_eq!(m.interface, InterfaceVersion { major: 1, minor: 0 });
        assert_eq!(m.version.to_string(), "0.2.0");
        let file = Sniff::new(b"\x18FICHIER GUITAR PRO v5.10 and so on");
        assert_eq!(m.format_for(&file).map(|f| f.id.as_str()), Some("gp5"));
        assert!(m.format_for(&Sniff::new(b"BCFZ")).is_none());
    }

    #[test]
    fn bad_manifests_say_why() {
        let err = |text: &str| Manifest::parse(text.as_bytes()).unwrap_err();
        assert!(err("").contains("empty"));
        assert!(err("[1]").contains("not valid"));
        assert!(
            Manifest::parse(&[0xff, 0xfe])
                .unwrap_err()
                .contains("UTF-8")
        );
        assert!(err(&GP5.replace("0.2.0", "0.2")).contains("not a version"));
        assert!(err(&GP5.replace("\"1.0\"", "\"1\"")).contains("interface version"));
        assert!(err(&GP5.replace("oxiguitar.gp5", "OxiGuitar GP5")).contains("id"));
        assert!(
            err(&GP5
                .replace("\"at\": 1, ", "")
                .replace("\"text\": \"FICHIER GUITAR PRO v5\"", ""))
            .contains("signature")
        );
        assert!(err(&GP5.replace("[\"gp5\"]", "[\".gp5\"]")).contains("extension"));
        assert!(err(&GP5.replace("\"read\": true,", "")).contains("neither"));
        let tabbed = GP5.replace(
            "\"formats\"",
            "\"tabs\": [{\"id\": \"t\", \"title\": \"T\", \"area\": \"left\"}], \"events\": [\"score\"], \"formats\"",
        );
        assert!(err(&tabbed).contains("tabs need interface 1.1"));
        let m = Manifest::parse(tabbed.replace("\"1.0\"", "\"1.1\"").as_bytes()).unwrap();
        assert!(m.wants(EventKind::Score) && !m.wants(EventKind::Playback));
        assert!(
            err(&tabbed
                .replace("\"1.0\"", "\"1.1\"")
                .replace("\"T\"", "\" \""))
            .contains("title")
        );
        let tooled = GP5.replace(
            "\"formats\"",
            "\"tools\": [{\"id\": \"up\", \"name\": \"Up an Octave\"}], \"formats\"",
        );
        assert!(err(&tooled).contains("tools need interface 1.2"));
        let m = Manifest::parse(tooled.replace("\"1.0\"", "\"1.2\"").as_bytes()).unwrap();
        assert_eq!(m.tools[0].name, "Up an Octave");
        assert!(
            err(&tooled
                .replace("\"1.0\"", "\"1.2\"")
                .replace("\"Up an Octave\"", "\"\""))
            .contains("name is empty")
        );
        let big = vec![b' '; crate::MAX_MANIFEST_LEN + 1];
        assert!(Manifest::parse(&big).unwrap_err().contains("more than"));
    }

    #[test]
    fn ids() {
        for ok in ["gp5", "oxiguitar.gp5", "a-b.c-d", "x1"] {
            assert!(is_id(ok), "{ok}");
        }
        for bad in ["", "Gp5", "a..b", ".a", "a.", "a_b", "a b", &"a".repeat(65)] {
            assert!(!is_id(bad), "{bad}");
        }
    }

    #[test]
    fn versions_order_like_semver() {
        let v = |s: &str| s.parse::<Version>().unwrap();
        assert!(v("0.2.0") > v("0.1.9"));
        assert!(v("1.0.0") > v("1.0.0-rc.1"));
        assert!(v("1.0.0-rc.2") > v("1.0.0-rc.1"));
        assert_eq!(v("10.20.30-dev.1").to_string(), "10.20.30-dev.1");
        for bad in [
            "1.0", "1.0.0.0", "01.0.0", "1.0.0-", "a.b.c", "-1.0.0", "1.+0.0",
        ] {
            assert!(bad.parse::<Version>().is_err(), "{bad}");
        }
    }

    #[test]
    fn interface_1_3_keys_are_checked() {
        let json = |interface: &str, extra: &str| {
            format!(
                r#"{{"interface": "{interface}", "id": "me.nam", "name": "NAM",
                    "version": "0.1.0" {extra}}}"#
            )
        };
        let keys = r#", "settings_window": true, "folders": ["nam", "soundfonts"],
            "track_sections": [{"id": "amp", "title": "Amp", "plays": ["fretted"]}],
            "conditions": {"arch": ["x86_64"], "cpu": ["avx2", "fma"]}"#;
        let m: Manifest = serde_json::from_str(&json("1.3", keys)).unwrap();
        assert!(m.problems().is_empty(), "{:?}", m.problems());
        assert_eq!(m.folders, [SharedFolder::Nam, SharedFolder::Soundfonts]);
        assert!(m.track_sections[0].shows_for(InstrumentClass::Fretted));
        assert!(!m.track_sections[0].shows_for(InstrumentClass::Drums));
        let old: Manifest = serde_json::from_str(&json("1.2", keys)).unwrap();
        assert_eq!(
            old.problems(),
            [
                "a settings window needs interface 1.3, and the manifest asks for 1.2",
                "track sections need interface 1.3, and the manifest asks for 1.2",
                "shared folders need interface 1.3, and the manifest asks for 1.2",
                "processor features (conditions \"cpu\") need interface 1.3, and the manifest asks for 1.2",
            ]
        );
        assert_eq!(m.conditions.cpu, ["avx2", "fma"]);
        let bad = r#", "folders": ["nam", "nam"],
            "track_sections": [{"id": "Amp", "title": " ", "plays": []}]"#;
        let m: Manifest = serde_json::from_str(&json("1.3", bad)).unwrap();
        assert_eq!(m.problems().len(), 4, "{:?}", m.problems());
    }

    #[test]
    fn instruments_play_classes() {
        let spec =
            |plays: Option<Vec<InstrumentClass>>, not: Vec<InstrumentClass>| InstrumentSpec {
                id: "i".into(),
                name: "I".into(),
                plays,
                does_not_play: not,
            };
        use InstrumentClass as C;
        assert!(spec(None, vec![]).plays(C::Drums));
        assert!(!spec(None, vec![C::Drums]).plays(C::Drums));
        let drums = spec(Some(vec![C::Drums]), vec![]);
        assert!(drums.plays(C::Drums) && !drums.plays(C::Fretted));
        let json = r#"{"id": "kit", "name": "Kit", "plays": ["drums"]}"#;
        assert_eq!(
            serde_json::from_str::<InstrumentSpec>(json).unwrap(),
            drums.clone().with_id("kit", "Kit")
        );
    }

    impl InstrumentSpec {
        fn with_id(mut self, id: &str, name: &str) -> InstrumentSpec {
            self.id = id.into();
            self.name = name.into();
            self
        }
    }

    #[test]
    fn conditions() {
        let env = Environment {
            os: "linux".into(),
            arch: "x86_64".into(),
            display: "wayland".into(),
            cpu: vec!["avx".into(), "avx2".into()],
        };
        assert_eq!(Conditions::default().unmet(&env), None);
        let v3 = |features: &[&str]| Conditions {
            cpu: features.iter().map(|f| (*f).to_owned()).collect(),
            ..Conditions::default()
        };
        assert_eq!(v3(&["avx2"]).unmet(&env), None);
        assert_eq!(
            v3(&["avx2", "fma"]).unmet(&env).as_deref(),
            Some("it needs the processor feature fma, which this processor doesn't have")
        );
        // A feature this host doesn't know is missing, never assumed.
        assert!(v3(&["avx1024"]).unmet(&env).is_some());
        let x11 = Conditions {
            display: vec!["x11".into()],
            ..Conditions::default()
        };
        assert_eq!(
            x11.unmet(&env).as_deref(),
            Some("it runs only with the display system x11, and this is wayland")
        );
        let either = Conditions {
            os: vec!["macos".into(), "linux".into()],
            ..Conditions::default()
        };
        assert_eq!(either.unmet(&env), None);
    }
}
