// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Playing the timeline's notes through rustysynth 1.3.8, as it is
//! published: one synthesiser per track, each rendering into its track's
//! buffers. A fretted track gives each string a channel of its own, so a
//! string's pitch moves alone (bends, slides, vibrato) and a new note on a
//! string ends the one before it, as on the instrument; other tracks share
//! one channel, and drum kits play on the General MIDI drum channel.
//!
//! The effects in MIDI terms (PLAY-3), through rustysynth's public API
//! only, every 8 frames:
//!
//! - **Bends, the whammy bar and slides** are the notes' pitch curves, as
//!   the timeline gives them, sent as pitch bend on the string's channel
//!   (a range of 24 semitones, so 0.3 cents a step; deeper dives stop
//!   there).
//! - **Vibrato** is a pitch curve of our own on top: 30 cents at 5.5 Hz
//!   for the fretting hand's, 60 cents at 4.5 Hz for the whammy bar's,
//!   reaching full depth over its first 0.1 s.
//! - **Hammer-ons, pull-offs, legato slides and trills** (notes not
//!   picked) carry on the note sounding on their string, bent to their
//!   pitch, instead of starting a new one: the string keeps ringing, as it
//!   does, with no new attack. Where there is none (or it is more than 24
//!   semitones away), the note starts softer.
//! - **Palm mutes** end after 0.15 s, a little softer, and **dead notes**
//!   after 30 ms, much softer: the set's own sound, damped.
//! - **Harmonics** sound at the harmonic's pitch (the timeline's key);
//!   natural, artificial and tapped ones on clean guitars play General
//!   MIDI's Guitar Harmonics (program 31). **Slapped and popped** notes on
//!   a bass play Slap Bass 1 and 2 (programs 36 and 37).
//! - **Fades**: a swell rises from silence over the note, a fade out
//!   falls over its second half (expression).
//! - Ghost notes, let ring, grace notes, strums, tremolo picking and
//!   trills come from the timeline as they are (softer, longer, earlier,
//!   repeated).
//!
//! rustysynth renders in blocks of 8 frames and starts a note at its next
//! block, so a note starts at most 7 frames after its event (0.15 ms at
//! 48 kHz, 0.16 ms at 44.1 kHz): the renderer declares that resolution.
//!
//! When everything stops at once (stop, seek, a loop's jump, a change of
//! speed), what sounds fades out over 2.5 ms instead of being cut, which
//! would click, while the notes that follow start at full level: each
//! track has a second synthesiser, and the one that sounded fades out while
//! the other takes over.
//!
//! Nothing here allocates, locks or waits once prepared: the synthesisers,
//! their voices and the scratch buffers are made in `Player::new`.

use std::sync::Arc;

use oxiguitar_plugin::audio::{
    Piece, PitchPoint, Prepare, PreparedTrack, RtEvent, RtEventKind, RtNote, attack, cents_at,
    flags, harmonic, sound,
};
use oxiguitar_plugin::{Error, ErrorKind, Renderer};
use rustysynth::{SoundFont, Synthesizer, SynthesizerSettings};

use crate::sfz::sampler::Sampler;

/// rustysynth's smallest block: notes start at most `BLOCK - 1` frames
/// late, and controllers change every `BLOCK` frames.
pub(crate) const BLOCK: usize = 8;

/// The most voices one track sounds at once (a note can take several: a
/// stereo pair, layers).
const POLYPHONY: usize = 128;

/// The most notes a renderer follows at once; later ones still sound but
/// can't be told apart when they end, so they end with their channel.
const NOTES: usize = 256;

/// The General MIDI drum channel (rustysynth's percussion channel).
const DRUM_CHANNEL: u8 = 9;

/// How long sound fades out when everything stops at once: 2.5 ms, the
/// rate over this.
const FADE_PER_SECOND: u32 = 400;

/// The pitch bend range, in semitones each way.
const BEND_RANGE: f64 = 24.0;

/// Vibrato: depth in cents and rate in hertz, for the fretting hand's and
/// the whammy bar's; and how long it takes to reach full depth.
const VIBRATO: (f64, f64) = (30.0, 5.5);
const WIDE_VIBRATO: (f64, f64) = (60.0, 4.5);
pub(crate) const VIBRATO_RISE: f64 = 0.1;

/// A note's vibrato, if it has one: its depth in cents and rate in hertz.
pub(crate) fn vibrato_of(note: &RtNote) -> Option<(f64, f64)> {
    if note.flags & flags::WIDE_VIBRATO != 0 {
        Some(WIDE_VIBRATO)
    } else if note.flags & flags::VIBRATO != 0 {
        Some(VIBRATO)
    } else {
        None
    }
}

/// How long a palm mute and a dead note sound, in seconds of the score, and
/// their velocity against the written one.
const PALM_MUTE: (f64, f32) = (0.15, 0.9);
const DEAD: (f64, f32) = (0.03, 0.6);

/// A note not picked with nothing to carry on starts this much softer.
const UNPICKED: f32 = 0.7;

/// A fade's quietest point: −60 dB.
const FADE_FLOOR: f64 = 0.001;

/// MIDI messages, by their status byte, and the controllers used.
mod midi {
    pub(super) const CONTROL: i32 = 0xB0;
    pub(super) const PROGRAM: i32 = 0xC0;
    pub(super) const PITCH_BEND: i32 = 0xE0;
    pub(super) const BANK_SELECT: i32 = 0x00;
    pub(super) const DATA_ENTRY: i32 = 0x06;
    pub(super) const DATA_ENTRY_FINE: i32 = 0x26;
    pub(super) const EXPRESSION: i32 = 0x0B;
    pub(super) const RPN_FINE: i32 = 0x64;
    pub(super) const RPN: i32 = 0x65;
    /// The centre of pitch bend's 14 bits.
    pub(super) const BEND_CENTRE: i32 = 8192;
}

/// General MIDI programs the effects switch to.
mod program {
    pub(super) const GUITAR_HARMONICS: u8 = 31;
    pub(super) const SLAP_BASS_1: u8 = 36;
    pub(super) const SLAP_BASS_2: u8 = 37;
}

/// A note sounding: which track and channel it went to, and the key
/// rustysynth plays it at (a note carried on keeps the first one's).
#[derive(Clone, Copy, Debug)]
struct Sounding {
    id: u32,
    track: u16,
    channel: u8,
    key: u8,
    /// Its end came, but a note not picked on its string takes it over in
    /// the same call.
    held: bool,
}

/// What a channel plays now, for its controllers.
#[derive(Clone, Copy, Debug)]
struct Voice {
    /// The note heard (the last one carried on, for its curve and flags).
    note: RtNote,
    /// Cents from the key rustysynth plays to the note's.
    offset: f64,
    /// The vibrato's phase, in radians, and its seconds so far.
    phase: f64,
    vibrating: f64,
    /// When it ends early (palm mute, dead note), in seconds of the score.
    cut: f64,
}

/// A channel's state on the playing synthesiser.
#[derive(Clone, Copy, Debug)]
struct Channel {
    voice: Option<Voice>,
    /// The pitch bend and expression last sent.
    bend: i32,
    expression: i32,
    /// Its program now.
    program: u8,
}

impl Channel {
    const fn new(program: u8) -> Channel {
        Channel {
            voice: None,
            bend: midi::BEND_CENTRE,
            expression: 127,
            program,
        }
    }
}

/// One track's synthesisers and how it maps strings to channels.
struct Track {
    /// Two alike: `live` plays, the other fades out what sounded when
    /// everything last stopped at once.
    synths: [Synthesizer; 2],
    live: usize,
    /// Frames left of the other synthesiser's fade.
    fading: usize,
    /// The channel of each string, the highest string first; empty when
    /// the track has no strings.
    strings: Vec<u8>,
    /// The channel for notes without a string.
    channel: u8,
    /// The track's own program.
    program: u8,
    /// The live synthesiser's channels.
    channels: [Channel; 16],
}

impl Track {
    fn synth(&mut self) -> &mut Synthesizer {
        &mut self.synths[self.live]
    }

    /// Everything stops at once: what sounds fades out over `fade`
    /// frames, on the synthesiser that played it, and the other takes
    /// over, its controllers as at the start.
    fn stop(&mut self, fade: usize) {
        let other = 1 - self.live;
        // A fade still going is cut short: it is quiet already.
        self.synths[other].note_off_all(true);
        self.synths[other].reset_all_controllers();
        let previous = self.channels;
        self.live = other;
        self.fading = fade;
        for (c, was) in self.channels.iter_mut().zip(previous) {
            *c = Channel::new(was.program);
        }
        // Programs a note switched to go back to the track's.
        for c in 0..16u8 {
            self.set_program(c, self.program);
        }
    }

    fn set_program(&mut self, channel: u8, program: u8) {
        if channel == DRUM_CHANNEL || self.channels[usize::from(channel)].program == program {
            return;
        }
        self.channels[usize::from(channel)].program = program;
        let live = self.live;
        self.synths[live].process_midi_message(
            i32::from(channel),
            midi::PROGRAM,
            i32::from(program),
            0,
        );
    }

    fn send_bend(&mut self, channel: u8, cents: f64) {
        let steps = (cents / (BEND_RANGE * 100.0) * f64::from(midi::BEND_CENTRE)).round() as i32;
        let value = (midi::BEND_CENTRE + steps).clamp(0, 16383);
        let c = &mut self.channels[usize::from(channel)];
        if c.bend != value {
            c.bend = value;
            let live = self.live;
            self.synths[live].process_midi_message(
                i32::from(channel),
                midi::PITCH_BEND,
                value & 0x7F,
                value >> 7,
            );
        }
    }

    fn send_expression(&mut self, channel: u8, value: i32) {
        let value = value.clamp(0, 127);
        let c = &mut self.channels[usize::from(channel)];
        if c.expression != value {
            c.expression = value;
            let live = self.live;
            self.synths[live].process_midi_message(
                i32::from(channel),
                midi::CONTROL,
                midi::EXPRESSION,
                value,
            );
        }
    }

    /// The channel a note goes to.
    fn channel_of(&self, note: &RtNote) -> u8 {
        match (note.string as usize).checked_sub(1) {
            Some(s) if !self.strings.is_empty() => self.strings[s.min(self.strings.len() - 1)],
            _ => self.channel,
        }
    }

    /// The program a note plays: the track's, unless its effect has a
    /// General MIDI sound of its own.
    fn program_for(&self, note: &RtNote) -> u8 {
        let p = self.program;
        let clean_guitar = (24..=28).contains(&p);
        let bass = (32..=35).contains(&p);
        if note.sound == sound::HARMONIC
            && clean_guitar
            && matches!(
                note.harmonic,
                harmonic::NATURAL | harmonic::ARTIFICIAL | harmonic::TAPPED
            )
        {
            program::GUITAR_HARMONICS
        } else if bass && note.attack == attack::SLAPPED {
            program::SLAP_BASS_1
        } else if bass && note.attack == attack::POPPED {
            program::SLAP_BASS_2
        } else {
            p
        }
    }
}

/// A note's run of the pitch points: its curve, empty for a steady pitch.
pub(crate) fn run<'a>(pitch: &'a [PitchPoint], note: &RtNote) -> &'a [PitchPoint] {
    let start = note.pitch_start as usize;
    pitch
        .get(start..start.saturating_add(note.pitch_len as usize))
        .unwrap_or(&[])
}

/// A fade's level at `t`: a swell rises from −60 dB to full over the note,
/// a fade out falls from full to −60 dB over its second half; full
/// otherwise. As expression, whose square rustysynth applies.
fn expression(note: &RtNote, t: f64) -> i32 {
    let len = (note.end - note.start).max(1e-6);
    let mut level = 1.0f64;
    if note.flags & flags::FADE_IN != 0 {
        let p = ((t - note.start) / len).clamp(0.0, 1.0);
        level *= FADE_FLOOR.powf(1.0 - p);
    }
    if note.flags & flags::FADE_OUT != 0 {
        let half = note.start + len / 2.0;
        let p = ((t - half) / (len / 2.0)).clamp(0.0, 1.0);
        level *= FADE_FLOOR.powf(p);
    }
    (127.0 * level.sqrt()).round() as i32
}

/// What plays a track: a SoundFont through rustysynth, or an SFZ
/// instrument through our sampler.
pub(crate) enum Sound {
    Font(Arc<SoundFont>),
    Sfz(Arc<crate::sfz::Loaded>),
}

/// A track as it plays.
enum Part {
    Synth(Box<Track>),
    Sampler {
        sampler: Box<Sampler>,
        /// It has strings: a string sounds one note at a time.
        strings: bool,
    },
}

/// A prepared instrument.
pub(crate) struct Player {
    tracks: Vec<Part>,
    /// The notes sounding, in the order they started.
    sounding: [Option<Sounding>; NOTES],
    /// Scratch buffers for one synthesiser's render, as long as the
    /// largest piece.
    left: Vec<f32>,
    right: Vec<f32>,
    /// The frames a fade lasts.
    fade: usize,
    /// Frames a second.
    rate: f64,
}

impl std::fmt::Debug for Player {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Player")
            .field("tracks", &self.tracks.len())
            .finish_non_exhaustive()
    }
}

/// The channels for a track with `strings` strings: 0 upwards, past the
/// drum channel. A track has at most 15 strings' channels; more strings
/// share the last.
fn string_channels(strings: usize) -> Vec<u8> {
    (0u8..16)
        .filter(|&c| c != DRUM_CHANNEL)
        .chain(std::iter::repeat(15))
        .take(strings)
        .collect()
}

/// Whether, among `events` from `from` on, a note not picked starts on
/// `note`'s string of its track: then `note`'s end is carried on rather
/// than ended.
fn carried_on(events: &[RtEvent], note: &RtNote) -> bool {
    note.string > 0
        && events.iter().any(|e| {
            e.kind == RtEventKind::NOTE_ON
                && e.note.attack == attack::LEGATO
                && e.note.track == note.track
                && e.note.string == note.string
        })
}

impl Player {
    /// Gets ready to play `prepare.tracks`, each with its `sounds`:
    /// synthesisers at `prepare.rate`, set to each track's program, or an
    /// SFZ instrument's sampler.
    pub(crate) fn new(prepare: &Prepare, sounds: &[Sound]) -> Result<Player, Error> {
        let mut settings = SynthesizerSettings::new(prepare.rate as i32);
        settings.block_size = BLOCK;
        settings.maximum_polyphony = POLYPHONY;
        // Dry: the sound is the instrument's, and a track's effects (the
        // amp, M4 slice 5) come after it.
        settings.enable_reverb_and_chorus = false;
        if sounds.len() != prepare.tracks.len() {
            return Err(Error::new(ErrorKind::Failed, "a sound set for each track"));
        }
        let tracks = prepare
            .tracks
            .iter()
            .zip(sounds)
            .enumerate()
            .map(|(i, (t, sound))| match sound {
                Sound::Font(font) => Ok(Part::Synth(Box::new(track(t, font, &settings)?))),
                Sound::Sfz(sfz) => Ok(Part::Sampler {
                    // Each track's own random numbers, the same on every
                    // play: renders are repeatable.
                    sampler: Box::new(Sampler::new(
                        sfz.clone(),
                        prepare.rate,
                        0x9E37_79B9_7F4A_7C15 ^ (i as u64 + 1),
                    )),
                    strings: !t.percussion && !t.tuning.is_empty(),
                }),
            })
            .collect::<Result<Vec<_>, Error>>()?;
        let frames = prepare.frames as usize;
        Ok(Player {
            tracks,
            sounding: [None; NOTES],
            left: vec![0.0; frames],
            right: vec![0.0; frames],
            fade: (prepare.rate / FADE_PER_SECOND).max(1) as usize,
            rate: f64::from(prepare.rate.max(1)),
        })
    }

    fn note_on(&mut self, note: &RtNote, pitch: &[PitchPoint]) {
        let track = match self.tracks.get_mut(note.track as usize) {
            Some(Part::Synth(track)) => track,
            Some(Part::Sampler { sampler, strings }) => {
                if *strings {
                    // A string sounds one note: a new one ends the one
                    // before.
                    sampler.end_string(note.string, pitch);
                }
                sampler.note_on(note, pitch);
                return;
            }
            None => return,
        };
        let channel = track.channel_of(note);
        let on_string = !track.strings.is_empty() && note.string > 0;
        let cut = match note.sound {
            sound::PALM_MUTE => note.start + PALM_MUTE.0,
            sound::DEAD => note.start + DEAD.0,
            _ => f64::INFINITY,
        };
        // Not picked: carry on the note sounding on the string.
        if on_string && note.attack == attack::LEGATO {
            let base = self
                .sounding
                .iter_mut()
                .flatten()
                .find(|s| s.track == note.track as u16 && s.channel == channel);
            if let Some(s) = base {
                let offset = (f64::from(note.key) - f64::from(s.key)) * 100.0;
                if offset.abs() <= BEND_RANGE * 100.0 {
                    s.id = note.id;
                    s.held = false;
                    let c = &mut track.channels[usize::from(channel)];
                    let (phase, vibrating) = c.voice.map_or((0.0, 0.0), |v| (v.phase, v.vibrating));
                    c.voice = Some(Voice {
                        note: *note,
                        offset,
                        phase,
                        vibrating,
                        cut,
                    });
                    return;
                }
            }
        }
        if on_string {
            // A string sounds one note: a new one ends the one before.
            for s in self.sounding.iter_mut() {
                if let Some(old) = *s
                    && old.track == note.track as u16
                    && old.channel == channel
                {
                    track
                        .synth()
                        .note_off(i32::from(channel), i32::from(old.key));
                    *s = None;
                }
            }
        }
        let scale = match note.sound {
            sound::PALM_MUTE => PALM_MUTE.1,
            sound::DEAD => DEAD.1,
            _ if note.attack == attack::LEGATO => UNPICKED,
            _ => 1.0,
        };
        let velocity = (f32::from(note.velocity) * scale).round().clamp(1.0, 127.0) as i32;
        if channel != DRUM_CHANNEL {
            let program = track.program_for(note);
            track.set_program(channel, program);
            // The channel's pitch and level as the note starts.
            track.send_bend(channel, cents_at(run(pitch, note), note.start));
            track.send_expression(channel, expression(note, note.start));
            track.channels[usize::from(channel)].voice = Some(Voice {
                note: *note,
                offset: 0.0,
                phase: 0.0,
                vibrating: 0.0,
                cut,
            });
        }
        track
            .synth()
            .note_on(i32::from(channel), i32::from(note.key), velocity);
        let sounding = Sounding {
            id: note.id,
            track: note.track as u16,
            channel,
            key: note.key,
            held: false,
        };
        if let Some(free) = self.sounding.iter_mut().find(|s| s.is_none()) {
            *free = Some(sounding);
        }
    }

    /// Ends the note `id`, unless another note of the same key on its
    /// channel still sounds (rustysynth would end both).
    fn end(&mut self, i: usize) {
        let Some(ending) = self.sounding[i].take() else {
            return;
        };
        let shared =
            self.sounding.iter().flatten().any(|s| {
                s.track == ending.track && s.channel == ending.channel && s.key == ending.key
            });
        if let Some(Part::Synth(track)) = self.tracks.get_mut(usize::from(ending.track)) {
            let c = &mut track.channels[usize::from(ending.channel)];
            if c.voice.is_some_and(|v| v.note.id == ending.id) {
                c.voice = None;
            }
            if !shared {
                track
                    .synth()
                    .note_off(i32::from(ending.channel), i32::from(ending.key));
            }
        }
    }

    fn note_off(&mut self, note: &RtNote, later: &[RtEvent], pitch: &[PitchPoint]) {
        if let Some(Part::Sampler { sampler, .. }) = self.tracks.get_mut(note.track as usize) {
            sampler.note_off(note.id, pitch);
            return;
        }
        let Some(i) = self
            .sounding
            .iter()
            .position(|s| s.is_some_and(|s| s.id == note.id))
        else {
            // Already ended by a later note on its string, or early (palm
            // mute, dead note), or never followed: nothing to end.
            return;
        };
        if carried_on(later, note) {
            if let Some(s) = &mut self.sounding[i] {
                s.held = true;
            }
            return;
        }
        self.end(i);
    }

    fn all_off(&mut self) {
        self.sounding = [None; NOTES];
        for t in &mut self.tracks {
            match t {
                Part::Synth(t) => t.stop(self.fade),
                Part::Sampler { sampler, .. } => sampler.stop(),
            }
        }
    }

    /// Every channel's pitch and level at `t` (seconds of the score), and
    /// the notes that end early ending; `seconds` of sound go by until the
    /// next update.
    fn update(&mut self, pitch: &[PitchPoint], t: f64, seconds: f64) {
        for ti in 0..self.tracks.len() {
            if let Part::Sampler { sampler, .. } = &mut self.tracks[ti] {
                sampler.update(pitch, t, seconds);
                continue;
            }
            for c in 0..16u8 {
                let Part::Synth(track) = &self.tracks[ti] else {
                    break;
                };
                let Some(mut v) = track.channels[usize::from(c)].voice else {
                    continue;
                };
                if t >= v.cut {
                    if let Some(i) = self
                        .sounding
                        .iter()
                        .position(|s| s.is_some_and(|s| s.id == v.note.id))
                    {
                        self.end(i);
                    }
                    if let Part::Synth(track) = &mut self.tracks[ti] {
                        track.channels[usize::from(c)].voice = None;
                    }
                    continue;
                }
                let curve = run(pitch, &v.note);
                let mut cents = v.offset + cents_at(curve, t);
                if let Some((depth, hz)) = vibrato_of(&v.note) {
                    let rise = (v.vibrating / VIBRATO_RISE).min(1.0);
                    cents += depth * rise * v.phase.sin();
                    v.phase =
                        (v.phase + std::f64::consts::TAU * hz * seconds) % std::f64::consts::TAU;
                    v.vibrating += seconds;
                }
                if let Part::Synth(track) = &mut self.tracks[ti] {
                    track.send_bend(c, cents);
                    track.send_expression(c, expression(&v.note, t));
                    track.channels[usize::from(c)].voice = Some(v);
                }
            }
        }
    }
}

/// The synthesisers for `t`, with every channel it uses set to its program
/// and, for strings, a pitch bend range of `BEND_RANGE`.
fn track(
    t: &PreparedTrack,
    font: &Arc<SoundFont>,
    settings: &SynthesizerSettings,
) -> Result<Track, Error> {
    let synth = || {
        Synthesizer::new(font, settings)
            .map_err(|e| Error::new(ErrorKind::Failed, format!("the synthesiser: {e}")))
    };
    let mut synths = [synth()?, synth()?];
    let (strings, channel) = if t.percussion {
        (Vec::new(), DRUM_CHANNEL)
    } else if t.tuning.is_empty() {
        (Vec::new(), 0)
    } else {
        (string_channels(t.tuning.len()), 0)
    };
    let program = t.program.min(127);
    let mut used: Vec<u8> = strings.clone();
    used.push(channel);
    used.sort_unstable();
    used.dedup();
    for synth in &mut synths {
        for &c in &used {
            let c = i32::from(c);
            if c != i32::from(DRUM_CHANNEL) {
                synth.process_midi_message(c, midi::CONTROL, midi::BANK_SELECT, 0);
                // Pitch bend range (registered parameter 0): BEND_RANGE
                // semitones.
                synth.process_midi_message(c, midi::CONTROL, midi::RPN, 0);
                synth.process_midi_message(c, midi::CONTROL, midi::RPN_FINE, 0);
                synth.process_midi_message(c, midi::CONTROL, midi::DATA_ENTRY, BEND_RANGE as i32);
                synth.process_midi_message(c, midi::CONTROL, midi::DATA_ENTRY_FINE, 0);
            }
            // On the drum channel rustysynth plays bank 128, and the
            // program is the kit (0, the standard kit, for Guitar Pro's).
            synth.process_midi_message(c, midi::PROGRAM, i32::from(program), 0);
        }
    }
    Ok(Track {
        synths,
        live: 0,
        fading: 0,
        strings,
        channel,
        program,
        channels: [Channel::new(program); 16],
    })
}

impl Renderer for Player {
    fn render(&mut self, piece: &mut Piece<'_, '_>) {
        let events = piece.events();
        // The pitch curves, as a whole: the notes' runs of it.
        let pitch = piece.pitch();
        for (i, e) in events.iter().enumerate() {
            match e.kind {
                RtEventKind::NOTE_ON => self.note_on(&e.note, pitch),
                RtEventKind::NOTE_OFF => self.note_off(&e.note, &events[i + 1..], pitch),
                RtEventKind::ALL_OFF => self.all_off(),
                _ => {}
            }
        }
        // Ends held for a note to carry them on that didn't.
        for i in 0..NOTES {
            if self.sounding[i].is_some_and(|s| s.held) {
                self.end(i);
            }
        }
        let frames = piece.frames().min(self.left.len());
        let (time, step) = (piece.time(), piece.step());
        let fade = self.fade;
        let mut f = 0;
        while f < frames {
            let n = (frames - f).min(BLOCK);
            self.update(pitch, time + f as f64 * step, n as f64 / self.rate);
            for (slot, part) in self.tracks.iter_mut().enumerate() {
                let Some((out_l, out_r)) = piece.track(slot) else {
                    continue;
                };
                let (out_l, out_r) = (&mut out_l[f..f + n], &mut out_r[f..f + n]);
                let t = match part {
                    Part::Synth(t) => t,
                    Part::Sampler { sampler, .. } => {
                        sampler.render(out_l, out_r);
                        continue;
                    }
                };
                let (l, r) = (&mut self.left[..n], &mut self.right[..n]);
                t.synths[t.live].render(l, r);
                for (o, v) in out_l.iter_mut().zip(l.iter()) {
                    *o += *v;
                }
                for (o, v) in out_r.iter_mut().zip(r.iter()) {
                    *o += *v;
                }
                if t.fading > 0 {
                    let other = 1 - t.live;
                    t.synths[other].render(l, r);
                    for i in 0..n {
                        let left = t.fading.saturating_sub(i);
                        let gain = left as f32 / fade as f32;
                        out_l[i] += l[i] * gain;
                        out_r[i] += r[i] * gain;
                    }
                    t.fading = t.fading.saturating_sub(n);
                    if t.fading == 0 {
                        t.synths[other].note_off_all(true);
                    }
                }
            }
            f += n;
        }
    }

    fn resolution(&self) -> u32 {
        (BLOCK - 1) as u32
    }
}

#[cfg(test)]
mod tests {
    use oxiguitar_plugin::api::class::InstrumentClass;
    use oxiguitar_plugin::api::message::TrackInfo;
    use oxiguitar_plugin::audio::RtEvent;

    use super::*;

    impl Player {
        fn synth(&self, i: usize) -> &Track {
            match &self.tracks[i] {
                Part::Synth(t) => t,
                Part::Sampler { .. } => panic!("track {i} is a sampler"),
            }
        }
    }

    fn font() -> Arc<SoundFont> {
        let bytes = crate::sf2::test_set();
        Arc::new(SoundFont::new(&mut std::io::Cursor::new(bytes)).unwrap())
    }

    fn guitar() -> PreparedTrack {
        PreparedTrack {
            info: TrackInfo {
                index: 0,
                name: "Guitar".into(),
                class: InstrumentClass::Fretted,
            },
            program: 29,
            percussion: false,
            tuning: vec![64, 59, 55, 50, 45, 40],
            capo: 0,
            settings: "{}".into(),
        }
    }

    fn prepare(tracks: Vec<PreparedTrack>) -> Prepare {
        Prepare {
            instrument: "soundfont".into(),
            rate: 48_000,
            frames: 256,
            tracks,
        }
    }

    fn note(id: u32, string: u8, key: u8) -> RtNote {
        RtNote {
            id,
            key,
            string,
            velocity: 100,
            ..RtNote::default()
        }
    }

    /// Renders `frames` frames of one track with `events`; returns left.
    fn render(p: &mut Player, events: &[RtEvent], frames: usize) -> Vec<f32> {
        let (mut l, mut r) = (vec![0.0; frames], vec![0.0; frames]);
        {
            let mut out = [&mut l[..], &mut r[..]];
            p.render(&mut Piece::new(
                events,
                &[],
                0.0,
                1.0 / 48_000.0,
                frames,
                &mut out,
            ));
        }
        l
    }

    fn on(n: RtNote) -> RtEvent {
        RtEvent {
            kind: RtEventKind::NOTE_ON,
            note: n,
        }
    }

    fn off(n: RtNote) -> RtEvent {
        RtEvent {
            kind: RtEventKind::NOTE_OFF,
            note: n,
        }
    }

    /// Plays `script` (events at their times, in seconds) for `seconds`
    /// of one guitar track, with `pitch` as the timeline's pitch points;
    /// returns the left channel and the player.
    fn play(
        script: &[(f64, RtEvent)],
        pitch: &[PitchPoint],
        seconds: f64,
        program: u8,
    ) -> (Vec<f32>, Player) {
        let mut t = guitar();
        t.program = program;
        let mut p = Player::new(&prepare(vec![t]), &[Sound::Font(font())]).unwrap();
        let step = 1.0 / 48_000.0;
        let total = (seconds * 48_000.0) as usize;
        let mut out = Vec::with_capacity(total);
        let mut at = 0usize;
        let mut next = 0;
        while at < total {
            let now = at as f64 * step;
            let mut events = Vec::new();
            while next < script.len() && script[next].0 <= now + 1e-9 {
                events.push(script[next].1);
                next += 1;
            }
            // Up to the next event, at most 256 frames.
            let until = script
                .get(next)
                .map_or(total, |(t, _)| ((t / step).round() as usize).max(at + 1));
            let n = (until - at).min(256).min(total - at);
            let (mut l, mut r) = (vec![0.0; n], vec![0.0; n]);
            {
                let mut bufs = [&mut l[..], &mut r[..]];
                p.render(&mut Piece::new(&events, pitch, now, step, n, &mut bufs));
            }
            out.extend_from_slice(&l);
            at += n;
        }
        (out, p)
    }

    /// The frequency of `sound` from `from` to `to` seconds, by rising zero
    /// crossings.
    fn hz(sound: &[f32], from: f64, to: f64) -> f64 {
        let part = &sound[(from * 48_000.0) as usize..(to * 48_000.0) as usize];
        let crossings = part
            .windows(2)
            .filter(|w| w[0] < 0.0 && w[1] >= 0.0)
            .count();
        crossings as f64 / (to - from)
    }

    fn peak(sound: &[f32], from: f64, to: f64) -> f32 {
        sound[(from * 48_000.0) as usize..(to * 48_000.0) as usize]
            .iter()
            .fold(0f32, |m, v| m.max(v.abs()))
    }

    #[test]
    fn a_bend_moves_the_strings_pitch() {
        // The test set's sine sounds 480 Hz at key 70.506; key 69 is 440 Hz.
        let pitch = [
            PitchPoint {
                time: 0.1,
                cents: 0,
            },
            PitchPoint {
                time: 0.2,
                cents: 1200,
            },
        ];
        let mut a = note(1, 1, 69);
        a.end = 1.0;
        a.pitch_len = 2;
        let (sound, _) = play(&[(0.0, on(a))], &pitch, 0.5, 29);
        let (low, high) = (hz(&sound, 0.0, 0.1), hz(&sound, 0.3, 0.5));
        assert!((low - 440.0).abs() < 15.0, "{low}");
        assert!((high - 880.0).abs() < 20.0, "{high}");
    }

    #[test]
    fn a_hammer_on_carries_on_the_strings_note() {
        let mut a = note(1, 1, 69);
        a.end = 0.2;
        let mut b = note(2, 1, 71);
        b.start = 0.2;
        b.end = 0.5;
        b.attack = attack::LEGATO;
        let (sound, p) = play(&[(0.0, on(a)), (0.2, off(a)), (0.2, on(b))], &[], 0.45, 29);
        let s: Vec<_> = p.sounding.iter().flatten().collect();
        assert_eq!(s.len(), 1);
        assert_eq!((s[0].id, s[0].key), (2, 69));
        // The same voice, a tone up: 440 Hz times 2^(2/12).
        let up = hz(&sound, 0.25, 0.45);
        assert!((up - 493.9).abs() < 15.0, "{up}");
        // No new attack: no silence and no jump in level at the change.
        assert!(peak(&sound, 0.2, 0.21) > peak(&sound, 0.15, 0.2) * 0.8);
    }

    #[test]
    fn a_hammer_on_with_nothing_to_carry_on_starts_softer() {
        let mut picked = note(1, 1, 69);
        picked.end = 0.5;
        let mut hammered = picked;
        hammered.attack = attack::LEGATO;
        let (a, _) = play(&[(0.0, on(picked))], &[], 0.2, 29);
        let (b, _) = play(&[(0.0, on(hammered))], &[], 0.2, 29);
        assert!(peak(&b, 0.1, 0.2) < peak(&a, 0.1, 0.2) * 0.85);
    }

    #[test]
    fn a_palm_mute_and_a_dead_note_end_early() {
        let mut muted = note(1, 1, 69);
        muted.end = 1.0;
        muted.sound = sound::PALM_MUTE;
        let (sound, p) = play(&[(0.0, on(muted))], &[], 0.4, 29);
        assert!(peak(&sound, 0.05, 0.1) > 0.01);
        assert!(peak(&sound, 0.3, 0.4) < 1e-4);
        assert_eq!(p.sounding.iter().flatten().count(), 0);
        let mut dead = muted;
        dead.sound = sound::DEAD;
        let (sound, _) = play(&[(0.0, on(dead))], &[], 0.4, 29);
        assert!(peak(&sound, 0.15, 0.4) < 1e-4);
    }

    #[test]
    fn a_swell_rises_from_silence_and_a_fade_out_falls() {
        let mut swell = note(1, 1, 69);
        swell.end = 1.0;
        swell.flags = flags::FADE_IN;
        let (sound, _) = play(&[(0.0, on(swell))], &[], 1.0, 29);
        assert!(peak(&sound, 0.0, 0.05) < peak(&sound, 0.9, 1.0) * 0.1);
        let mut fading = swell;
        fading.flags = flags::FADE_OUT;
        let (sound, _) = play(&[(0.0, on(fading))], &[], 1.0, 29);
        assert!(peak(&sound, 0.9, 1.0) < peak(&sound, 0.1, 0.4) * 0.1);
        assert!(peak(&sound, 0.1, 0.4) > 0.01);
    }

    #[test]
    fn vibrato_moves_the_pitch_both_ways() {
        let mut v = note(1, 1, 69);
        v.end = 1.0;
        v.flags = flags::VIBRATO;
        let (sound, _) = play(&[(0.0, on(v))], &[], 1.0, 29);
        // Over quarters of a 5.5 Hz cycle the pitch differs by up to 60
        // cents (3.5 %): measured over 40 ms windows, the highest and lowest
        // are apart.
        let rates: Vec<f64> = (0..20)
            .map(|i| hz(&sound, 0.2 + i as f64 * 0.04, 0.24 + i as f64 * 0.04))
            .collect();
        let (lo, hi) = rates
            .iter()
            .fold((f64::MAX, 0f64), |(l, h), r| (l.min(*r), h.max(*r)));
        assert!(hi - lo > 20.0, "{rates:?}");
        let (steady, _) = play(&[(0.0, on(note(1, 1, 69)))], &[], 1.0, 29);
        let flat: Vec<f64> = (0..20)
            .map(|i| hz(&steady, 0.2 + i as f64 * 0.04, 0.24 + i as f64 * 0.04))
            .collect();
        let (lo, hi) = flat
            .iter()
            .fold((f64::MAX, 0f64), |(l, h), r| (l.min(*r), h.max(*r)));
        assert!(hi - lo <= 25.0 + 1e-9, "{flat:?}");
    }

    #[test]
    fn harmonics_and_slaps_switch_program_and_switch_back() {
        let mut h = note(1, 1, 76);
        h.end = 0.1;
        h.sound = sound::HARMONIC;
        h.harmonic = harmonic::NATURAL;
        let mut plain = note(2, 1, 64);
        plain.start = 0.1;
        plain.end = 0.2;
        let (_, p) = play(&[(0.0, on(h))], &[], 0.05, 25);
        assert_eq!(p.synth(0).channels[0].program, program::GUITAR_HARMONICS);
        let (_, p) = play(
            &[(0.0, on(h)), (0.1, off(h)), (0.1, on(plain))],
            &[],
            0.15,
            25,
        );
        assert_eq!(p.synth(0).channels[0].program, 25);
        // A distorted guitar's harmonic keeps its own sound.
        let (_, p) = play(&[(0.0, on(h))], &[], 0.05, 30);
        assert_eq!(p.synth(0).channels[0].program, 30);
        let mut slap = note(3, 1, 40);
        slap.end = 0.1;
        slap.attack = attack::SLAPPED;
        let (_, p) = play(&[(0.0, on(slap))], &[], 0.05, 33);
        assert_eq!(p.synth(0).channels[0].program, program::SLAP_BASS_1);
    }

    #[test]
    fn strings_get_channels_of_their_own_past_the_drums() {
        assert_eq!(string_channels(6), [0, 1, 2, 3, 4, 5]);
        assert_eq!(string_channels(11), [0, 1, 2, 3, 4, 5, 6, 7, 8, 10, 11]);
        assert_eq!(string_channels(17).len(), 17);
        assert_eq!(string_channels(17)[16], 15);
    }

    #[test]
    fn a_note_sounds_within_the_resolution_and_stops_after_its_release() {
        let mut p = Player::new(&prepare(vec![guitar()]), &[Sound::Font(font())]).unwrap();
        assert!(render(&mut p, &[], 64).iter().all(|v| *v == 0.0));
        let a = note(1, 1, 64);
        let l = render(&mut p, &[on(a)], 64);
        let first = l.iter().position(|v| v.abs() > 1e-4).unwrap();
        assert!(first < BLOCK, "first sound on frame {first}");
        let _ = render(&mut p, &[off(a)], 64);
        // The test set's release is short (6000 timecents below a second).
        let mut tail = Vec::new();
        for _ in 0..40 {
            tail = render(&mut p, &[], 256);
        }
        assert!(tail.iter().all(|v| v.abs() < 1e-4));
    }

    #[test]
    fn a_new_note_on_a_string_ends_the_one_before() {
        let mut p = Player::new(&prepare(vec![guitar()]), &[Sound::Font(font())]).unwrap();
        let (a, b) = (note(1, 2, 60), note(2, 2, 62));
        let _ = render(&mut p, &[on(a)], 64);
        let _ = render(&mut p, &[on(b)], 64);
        assert_eq!(p.sounding.iter().flatten().count(), 1);
        assert_eq!(p.sounding.iter().flatten().next().map(|s| s.id), Some(2));
        // The first note's own end changes nothing now.
        let _ = render(&mut p, &[off(a)], 64);
        assert_eq!(p.sounding.iter().flatten().count(), 1);
    }

    #[test]
    fn stopping_fades_out_at_once_and_the_next_notes_start_at_full_level() {
        let mut p = Player::new(&prepare(vec![guitar()]), &[Sound::Font(font())]).unwrap();
        let a = note(1, 1, 64);
        let _ = render(&mut p, &[on(a)], 256);
        let full = render(&mut p, &[], 256);
        let level = full.iter().fold(0f32, |m, v| m.max(v.abs()));
        let stop = RtEvent {
            kind: RtEventKind::ALL_OFF,
            note: RtNote::default(),
        };
        // 2.5 ms at 48 kHz: 120 frames, then silence.
        let faded = render(&mut p, &[stop], 256);
        assert!(faded[..8].iter().any(|v| v.abs() > level / 2.0));
        assert!(
            faded[120..].iter().all(|v| v.abs() < 1e-6),
            "{:?}",
            &faded[118..130]
        );
        // A note right after a stop, on the same piece, sounds fully.
        let _ = render(&mut p, &[on(a)], 64);
        let _ = render(&mut p, &[stop, on(note(2, 1, 64))], 256);
        let after = render(&mut p, &[], 256);
        let again = after.iter().fold(0f32, |m, v| m.max(v.abs()));
        assert!(
            (again - level).abs() < level * 0.05,
            "{again} against {level}"
        );
    }

    #[test]
    fn notes_on_other_strings_ring_together() {
        let mut p = Player::new(&prepare(vec![guitar()]), &[Sound::Font(font())]).unwrap();
        let _ = render(&mut p, &[on(note(1, 1, 64)), on(note(2, 2, 59))], 64);
        assert_eq!(p.sounding.iter().flatten().count(), 2);
    }

    #[test]
    fn two_tracks_render_into_their_own_buffers() {
        let mut bass = guitar();
        bass.program = 33;
        bass.tuning = vec![43, 38, 33, 28];
        let mut p = Player::new(
            &prepare(vec![guitar(), bass]),
            &[Sound::Font(font()), Sound::Font(font())],
        )
        .unwrap();
        let mut n = note(1, 1, 40);
        n.track = 1;
        let (mut b0, mut b1, mut b2, mut b3) =
            (vec![0.0; 64], vec![0.0; 64], vec![0.0; 64], vec![0.0; 64]);
        {
            let mut out = [&mut b0[..], &mut b1[..], &mut b2[..], &mut b3[..]];
            p.render(&mut Piece::new(
                &[on(n)],
                &[],
                0.0,
                1.0 / 48_000.0,
                64,
                &mut out,
            ));
        }
        assert!(b0.iter().all(|v| *v == 0.0));
        assert!(b2.iter().any(|v| *v != 0.0));
    }
}
