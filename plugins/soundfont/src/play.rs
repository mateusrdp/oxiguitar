// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! Playing the timeline's notes through rustysynth 1.3.8, as it is
//! published: one synthesiser per track, each rendering into its track's
//! buffers. A fretted track gives each string a channel of its own, so a
//! string's pitch can later move alone (bends, slides) and a new note on a
//! string ends the one before it, as on the instrument; other tracks share
//! one channel, and drum kits play on the General MIDI drum channel.
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
//! Nothing here allocates, locks or waits once prepared: the synthesisers,
//! their voices and the scratch buffers are made in `Player::new`.

use std::sync::Arc;

use oxiguitar_plugin::audio::{Piece, Prepare, PreparedTrack, RtEventKind, RtNote};
use oxiguitar_plugin::{Error, ErrorKind, Renderer};
use rustysynth::{SoundFont, Synthesizer, SynthesizerSettings};

/// rustysynth's smallest block: notes start at most `BLOCK - 1` frames late.
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

/// MIDI messages, by their status byte.
mod midi {
    pub(super) const CONTROL: i32 = 0xB0;
    pub(super) const PROGRAM: i32 = 0xC0;
    pub(super) const BANK_SELECT: i32 = 0x00;
}

/// A note sounding: which track and channel it went to, its key.
#[derive(Clone, Copy, Debug)]
struct Sounding {
    id: u32,
    track: u16,
    channel: u8,
    key: u8,
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
}

impl Track {
    fn synth(&mut self) -> &mut Synthesizer {
        &mut self.synths[self.live]
    }

    /// Everything stops at once: what sounds fades out over `fade`
    /// frames, on the synthesiser that played it.
    fn stop(&mut self, fade: usize) {
        let other = 1 - self.live;
        // A fade still going is cut short: it is quiet already.
        self.synths[other].note_off_all(true);
        self.synths[other].reset_all_controllers();
        self.live = other;
        self.fading = fade;
    }
}

/// A prepared instrument.
pub(crate) struct Player {
    tracks: Vec<Track>,
    /// The notes sounding, in the order they started.
    sounding: [Option<Sounding>; NOTES],
    /// Scratch buffers for one synthesiser's render, as long as the
    /// largest piece.
    left: Vec<f32>,
    right: Vec<f32>,
    /// The frames a fade lasts.
    fade: usize,
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

impl Player {
    /// Gets ready to play `prepare.tracks` with `font`: one synthesiser
    /// per track at `prepare.rate`, set to each track's program.
    pub(crate) fn new(prepare: &Prepare, font: &Arc<SoundFont>) -> Result<Player, Error> {
        let mut settings = SynthesizerSettings::new(prepare.rate as i32);
        settings.block_size = BLOCK;
        settings.maximum_polyphony = POLYPHONY;
        // Dry: the sound is the instrument's, and a track's effects (the
        // amp, M4 slice 5) come after it.
        settings.enable_reverb_and_chorus = false;
        let tracks = prepare
            .tracks
            .iter()
            .map(|t| track(t, font, &settings))
            .collect::<Result<Vec<_>, Error>>()?;
        let frames = prepare.frames as usize;
        Ok(Player {
            tracks,
            sounding: [None; NOTES],
            left: vec![0.0; frames],
            right: vec![0.0; frames],
            fade: (prepare.rate / FADE_PER_SECOND).max(1) as usize,
        })
    }

    fn note_on(&mut self, note: &RtNote) {
        let Some(track) = self.tracks.get_mut(note.track as usize) else {
            return;
        };
        let channel = match (note.string as usize).checked_sub(1) {
            Some(s) if !track.strings.is_empty() => track.strings[s.min(track.strings.len() - 1)],
            _ => track.channel,
        };
        if !track.strings.is_empty() && note.string > 0 {
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
        track.synth().note_on(
            i32::from(channel),
            i32::from(note.key),
            i32::from(note.velocity.clamp(1, 127)),
        );
        let sounding = Sounding {
            id: note.id,
            track: note.track as u16,
            channel,
            key: note.key,
        };
        if let Some(free) = self.sounding.iter_mut().find(|s| s.is_none()) {
            *free = Some(sounding);
        }
    }

    fn note_off(&mut self, note: &RtNote) {
        let Some(i) = self
            .sounding
            .iter()
            .position(|s| s.is_some_and(|s| s.id == note.id))
        else {
            // Already ended by a later note on its string, or never
            // followed: nothing to end.
            return;
        };
        let Some(ending) = self.sounding[i].take() else {
            return;
        };
        // Another note of the same key on the same channel still sounds
        // (let ring on a channel that strings share): rustysynth would end
        // both, so this one rings on with it.
        let shared =
            self.sounding.iter().flatten().any(|s| {
                s.track == ending.track && s.channel == ending.channel && s.key == ending.key
            });
        if !shared && let Some(track) = self.tracks.get_mut(usize::from(ending.track)) {
            track
                .synth()
                .note_off(i32::from(ending.channel), i32::from(ending.key));
        }
    }

    fn all_off(&mut self) {
        self.sounding = [None; NOTES];
        for t in &mut self.tracks {
            t.stop(self.fade);
        }
    }
}

/// A synthesiser for `t`, with every channel it uses set to its program.
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
    let program = i32::from(t.program.min(127));
    let mut used: Vec<u8> = strings.clone();
    used.push(channel);
    used.sort_unstable();
    used.dedup();
    for synth in &mut synths {
        for &c in &used {
            let c = i32::from(c);
            if c != i32::from(DRUM_CHANNEL) {
                synth.process_midi_message(c, midi::CONTROL, midi::BANK_SELECT, 0);
            }
            // On the drum channel rustysynth plays bank 128, and the
            // program is the kit (0, the standard kit, for Guitar Pro's).
            synth.process_midi_message(c, midi::PROGRAM, program, 0);
        }
    }
    Ok(Track {
        synths,
        live: 0,
        fading: 0,
        strings,
        channel,
    })
}

impl Renderer for Player {
    fn render(&mut self, piece: &mut Piece<'_, '_>) {
        for e in piece.events() {
            match e.kind {
                RtEventKind::NOTE_ON => self.note_on(&e.note),
                RtEventKind::NOTE_OFF => self.note_off(&e.note),
                RtEventKind::ALL_OFF => self.all_off(),
                _ => {}
            }
        }
        let frames = piece.frames().min(self.left.len());
        if frames == 0 {
            return;
        }
        let fade = self.fade;
        for (slot, t) in self.tracks.iter_mut().enumerate() {
            let Some((out_l, out_r)) = piece.track(slot) else {
                continue;
            };
            let (l, r) = (&mut self.left[..frames], &mut self.right[..frames]);
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
                for i in 0..frames {
                    let left = t.fading.saturating_sub(i);
                    let gain = left as f32 / fade as f32;
                    out_l[i] += l[i] * gain;
                    out_r[i] += r[i] * gain;
                }
                t.fading = t.fading.saturating_sub(frames);
                if t.fading == 0 {
                    t.synths[other].note_off_all(true);
                }
            }
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

    #[test]
    fn strings_get_channels_of_their_own_past_the_drums() {
        assert_eq!(string_channels(6), [0, 1, 2, 3, 4, 5]);
        assert_eq!(string_channels(11), [0, 1, 2, 3, 4, 5, 6, 7, 8, 10, 11]);
        assert_eq!(string_channels(17).len(), 17);
        assert_eq!(string_channels(17)[16], 15);
    }

    #[test]
    fn a_note_sounds_within_the_resolution_and_stops_after_its_release() {
        let mut p = Player::new(&prepare(vec![guitar()]), &font()).unwrap();
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
        let mut p = Player::new(&prepare(vec![guitar()]), &font()).unwrap();
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
        let mut p = Player::new(&prepare(vec![guitar()]), &font()).unwrap();
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
        let mut p = Player::new(&prepare(vec![guitar()]), &font()).unwrap();
        let _ = render(&mut p, &[on(note(1, 1, 64)), on(note(2, 2, 59))], 64);
        assert_eq!(p.sounding.iter().flatten().count(), 2);
    }

    #[test]
    fn two_tracks_render_into_their_own_buffers() {
        let mut bass = guitar();
        bass.program = 33;
        bass.tuning = vec![43, 38, 33, 28];
        let mut p = Player::new(&prepare(vec![guitar(), bass]), &font()).unwrap();
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
