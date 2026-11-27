//! SoundFont drum synthesis, the step sequencer and the audio output.
//!
//! The interface thread owns an [`Engine`] and sends it commands. The audio callback owns the
//! synthesizer and the sequencer, and takes commands only through a lock-free queue, so it never
//! blocks or allocates.
//!
//! # Threads
//!
//! [`Engine`] holds the output stream. On Android that is an AAudio stream opened directly, which
//! asks for low-latency mode and falls back to the default mode (see the `aaudio` module); it is
//! `Send`. Elsewhere it is a cpal `Stream`. In cpal 0.18.2 the stream is `Send` and `Sync` on
//! WASAPI (Windows) and WebAudio, and `Send` on CoreAudio, so `Engine` is `Send` on those
//! platforms. Do not rely on it being `Send` everywhere: create the engine on the thread that
//! will keep it, and drop it there. Dropping the engine stops the audio.
//!
//! # Patterns and song
//!
//! The sequencer has [`MAX_PATTERNS`] pattern slots. Each slot holds a velocity for each of
//! [`MAX_STEPS`] steps of each of [`MAX_ROWS`] rows, where velocity 0 means off, and a length in
//! steps. The rows, and so the drum key of each row, are shared by all patterns. All of this
//! storage is allocated once, when the engine starts, and lives in the audio callback. The
//! interface changes it with [`Engine::set_row_key`], [`Engine::set_step`] and
//! [`Engine::set_length`]. [`Engine::clear_pattern`] turns off every step of one slot in a single
//! command, which the interface uses before it sends a pattern again after rows were reordered or
//! removed.
//!
//! The song is a list of up to [`MAX_SONG`] pattern slots, set as a whole with
//! [`Engine::set_song`], and may loop ([`Engine::set_song_loop`]).
//!
//! The sequencer plays in one of two modes:
//!
//! - Pattern mode ([`Engine::play_pattern`]) loops one pattern. [`Engine::cue_pattern`] switches
//!   to another pattern at the next bar boundary (a bar is 16 steps, and the end of the pattern
//!   is a boundary too). Play goes on in the new pattern at the bar after the current one, wrapped
//!   to the new pattern's bar count, so the change is heard within a bar and stays in time.
//!   [`Engine::set_loop_bar`] limits play to one bar of the pattern: the step after the bar's
//!   last step is the bar's first step. If play is outside the bar when it is set, play moves
//!   into the bar at the next step, at the same step within a bar. A play call that starts
//!   outside the bar starts inside it the same way. A cued pattern starts at the first step of
//!   its own copy of the bar. A bar the pattern does not have, for example after the pattern got
//!   shorter, counts as no loop bar. The loop bar stays set across play, cue and stop until it
//!   is cleared; playing the song clears it.
//! - Song mode ([`Engine::play_song`]) plays each entry's pattern once through its own length,
//!   then the next entry. After the last entry it starts again at entry 0 if the song loops, and
//!   otherwise stops at the end of the last step; sounds already started ring out.
//!
//! The next position is worked out only when the next step is due, from the step that played
//! last and the settings at that moment. So every change counts from the next step on:
//!
//! - A shorter length in pattern mode wraps the position: the step after step `s` is
//!   `(s + 1) % length`, moved into the loop bar if one is set. In song mode a length at or below the current position ends the entry.
//! - A new song order keeps the current entry playing to its end. Then play goes on at the next
//!   index of the new order. If that index is past the end of the new order, play wraps to
//!   entry 0 when the song loops and stops otherwise. The same holds for turning the loop off.
//!
//! The tempo is set with [`Engine::set_tempo`]. A beat is four steps, so a step lasts
//! `sample_rate * 60 / (bpm * 4)` frames. The sequencer keeps that figure as `f64` and computes
//! each step's due frame by multiplication from a fixed origin, not by adding up rounded step
//! lengths, so the tempo does not drift over a long run, also across pattern and song changes. A
//! step sounds at the start of the 64-frame synthesizer block that contains its due frame, which
//! is at most 1.3 ms early at 48 kHz. A tempo change keeps the position within the current step
//! in proportion.
//!
//! Swing ([`Engine::set_swing`], 0.0 to [`MAX_SWING`]) delays every odd step of a pattern (1, 3,
//! 5 and so on, counted from the pattern's step 0, which always starts a bar) by that fraction of
//! a step; 0.33 is a triplet shuffle. The delay is added to the step's due frame, not to the
//! grid, so the even steps stay where they were and swing cannot cause drift. A swing or tempo
//! change applies to the next step, which fires exactly once, even if the change comes between
//! its straight and its swung time. The playhead reports the step from the frame it fired.
//!
//! [`Engine::playhead`] reports the pattern, step and song entry the listener hears now. The audio
//! callback keeps the frames at which the last few steps started, subtracts the output latency
//! from the frame it is about to output, and publishes the position that started at or before the
//! result to an atomic.
//!
//! # Preview track
//!
//! [`Engine::preview`] loops a list of hits ([`PreviewHit`]) on a hidden extra track, so a person
//! can hear an imported groove before it goes into the project. The track has its own storage:
//! a velocity for each of the 128 drum keys at each of [`MAX_STEPS`] steps, allocated once when
//! the engine starts. The keys play directly, not through the rows, so row keys and row gains do
//! not apply. The project's patterns, rows and song are never touched.
//!
//! The preview is a third play mode of the same sequencer and clock. So it plays at the engine's
//! tempo and swing, with the current kit, and swing counts from the preview's step 0. Starting a
//! preview stops the project's play. Starting the project's play, or [`Engine::stop`], stops the
//! preview. Its notes get note-offs and go to MIDI out like any other note.
//!
//! While the preview plays, the sequencer is not playing a project pattern: [`Engine::playhead`]
//! reports `None` and [`Engine::is_playing`] reports `false`. [`Engine::previewing`] reports the
//! preview instead.
//!
//! The hits travel as one command each. Before them goes a command that stops the old preview and
//! clears its hits; after them goes the command that starts the new one. A long preview may not
//! fit in the queue at once. The rest then waits in the backlog like any other command (see
//! below), so the preview starts a little later but complete.
//!
//! # Command queue
//!
//! The queue holds [`QUEUE_CAPACITY`] commands, and the audio callback empties it at every block.
//! A drum hit is dropped when the queue is full, since a late hit is worse than none. Every other
//! command waits in a backlog on the interface side and is sent, in order, by the next engine call
//! or by [`Engine::flush`], so a pattern change is delayed but never lost. Because the order is
//! kept, a [`Engine::play_song`] sent after [`Engine::set_song`] always plays the new song.
//!
//! # MIDI in
//!
//! Hits from a MIDI input travel through two more lock-free queues of [`LIVE_QUEUE_CAPACITY`]
//! entries each. The MIDI input's thread pushes [`LiveHit`]s into the first, whose sending end
//! [`Engine::take_live_hits`] hands out. The audio callback empties it at every block, after the
//! commands. It plays each hit as [`Engine::hit`] would, with its note-off and its MIDI out
//! message, but at the velocity scaled by the gain of the first row with the hit's key, so a
//! muted row stays silent. A hit whose `sound` is false is not played.
//!
//! Every hit, played or not, goes back to the interface through the second queue as a
//! [`HeardHit`], which [`Engine::take_heard_hits`] collects. It carries the position the player
//! was hearing when they hit. The callback works it out from the hit's time: the first frame of
//! its buffer is heard one output latency after the callback starts, the same relation MIDI out
//! uses, which gives the frame heard at the hit's time. The step history then gives the step
//! whose straight grid time is the newest at or before that frame. The position counts in
//! straight time, as if there were no swing, so the interface can snap to the swung grid itself.
//! A hit from before the history, or from when the project was not playing, has no position.
//!
//! # Row gain
//!
//! Each row has a gain from 0.0 to 1.0, set with [`Engine::set_row_gain`] and 1.0 by default. A
//! sequenced note of the row plays at velocity `round(velocity * gain)`, and a result of 0 is not
//! played at all. That is how mute works: the interface sends gain 0 for a muted row, and works
//! out solo itself. Gain does not apply to [`Engine::hit`]; the interface scales previews itself.
//! It does apply to hits from the MIDI input (see "MIDI in" below), through the first row with
//! the hit's key.
//!
//! The gain scales the velocity linearly, with no extra curve, because the synthesizer already
//! applies one. Its default SoundFont modulator turns velocity into attenuation along the SF2
//! concave curve, `40 * log10(127 / velocity)` dB, so the amplitude follows the square of the
//! velocity. A linear gain therefore acts like a fader marked in decibels: 0.5 is about -12 dB,
//! 0.25 about -24 dB and 0.1 about -40 dB. Perceived loudness roughly halves for every 10 dB, so
//! a slider at 0.5 sounds about half as loud. A curve such as `gain^0.5` would put half the slider
//! at only -6 dB and crowd all the quiet settings into the bottom of the slider. One side effect
//! comes with any curve: in a bank with velocity layers, a quieter row may also switch to a softer
//! sample.
//!
//! A gain change counts from the next step that fires. Notes already sounding are not changed.
//!
//! # Volume
//!
//! [`Engine::set_volume`] sets a gain from 0.0 to 1.0, 1.0 by default, that scales every sample
//! the engine outputs: after the synthesizer and the fade of [`Engine::silence`], before the
//! device. Unlike row gain it scales the sound, not the velocity, so a bank with velocity layers
//! plays the same samples at every volume. A new gain is reached over one block, by a linear ramp
//! from the old one, so moving a slider does not click. It does not apply to MIDI out (see
//! [`Engine::set_midi_volume`]) or to offline rendering, which has no command for it.
//!
//! # Kits
//!
//! A kit is a preset in the percussion bank of the SoundFont, which is bank 128 in rustysynth's
//! numbering. [`drum_kits`] lists them, and [`Engine::kits`] keeps the list of the bank the engine
//! started with. [`Engine::set_kit`] sends a program change on the drum channel. Previews from
//! [`Engine::hit`] and sequenced notes play on that same channel, so the kit changes both, from
//! the next note on. Notes already sounding keep their kit.
//!
//! The synthesizer looks the preset up at every note-on. If the bank has no preset 128:program, it
//! plays the standard kit, 128:0. If that is missing too, it plays the bank's default preset, the
//! one with the lowest bank and program (usually a piano at 0:0). Where the bank holds two presets
//! with the same bank and program, the synthesizer plays the last one in the file, while
//! [`drum_kits`] lists the first one's name.
//!
//! Every SoundFont has its own kits. [`resolve_kit`] picks the kit to play for one a project asks
//! for, and [`kit_keys`] says which keys a kit has a sound for.
//!
//! # Drum note-off
//!
//! The synthesizer honors note-off on the percussion channel like on any other channel: the voice
//! enters its release stage. Most General MIDI drum samples are not looped and end on their own,
//! but some banks loop a cymbal or a hi-hat, which would then sound forever. Sending note-off right
//! after note-on is not an option, because a drum region with a short release would be cut off at
//! once. So the renderer sends note-off [`RING_SECONDS`] after the most recent hit on a key, whether
//! the hit came from [`Engine::hit`] or from a pattern. A new hit on the same key moves that
//! deadline, so a note-off never cuts a fresh hit.
//!
//! Some kits have sounds whose release lasts many seconds after that note-off. So
//! [`Engine::stop`] also silences: the renderer fades the whole output to nothing over
//! [`SILENCE_SECONDS`], then resets the synthesizer, which ends every voice and the reverb and
//! chorus tails, and selects the kit again. A hit or a play that comes during the fade ends it at
//! once, so the new sound is heard whole. [`Engine::silence`] does the same without stopping.
//! The end of a song that does not loop does not silence: its last hits ring out.
//!
//! # Offline rendering
//!
//! [`OfflineRender`] runs the same renderer, sequencer and synthesizer without an audio device,
//! for WAV export. Its setters match the engine's but apply at once. It renders the song once, or
//! one pattern a number of times, plus a tail for the last hits to ring out, as fast as the
//! processor allows (about 200 times real time on a desktop). Each render starts from a silent
//! synthesizer at frame 0, so the result is the same every time. [`write_wav`] writes the result
//! as a 16-bit stereo WAV file.

use std::collections::VecDeque;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::time::{Duration, Instant};

#[cfg(not(target_os = "android"))]
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample};
#[cfg(not(target_os = "android"))]
use cpal::{SampleFormat, SizedSample};
use rustysynth::{Synthesizer, SynthesizerSettings};

pub use rustysynth::SoundFont;

#[cfg(target_os = "android")]
mod aaudio;
mod live;
mod midi;
mod offline;
pub use live::{HeardHit, HeardPosition, LIVE_QUEUE_CAPACITY, LiveHit, LiveHits};
pub use midi::{MIDI_QUEUE_CAPACITY, MidiEvent, MidiEvents};
pub use offline::{OfflineRender, write_wav, write_wav_to};

/// How long a drum hit may sound before the renderer sends its note-off.
pub const RING_SECONDS: f32 = 4.0;

/// How long after a hit MIDI out sends its note-off. Drum modules play the whole sound anyway,
/// but some devices hold a voice until the note-off comes. A keyboard that got note-ons with no
/// note-offs for a few seconds went silent while the beat played; see "Milestone 9: MIDI out" in
/// `docs/DESIGN.md`.
pub const MIDI_NOTE_SECONDS: f32 = 0.1;

/// How long [`Engine::stop`] takes to fade out the sounds still ringing. Short enough to feel
/// instant, long enough not to click.
pub const SILENCE_SECONDS: f32 = 0.02;

/// The most drum rows a project can have. All patterns share the rows.
pub const MAX_ROWS: usize = 32;
/// The most steps a pattern can loop over: 16 bars of 16 steps.
pub const MAX_STEPS: usize = 256;
/// The number of pattern slots. The interface names them A to Z.
pub const MAX_PATTERNS: usize = 26;
/// The most entries a song can have.
pub const MAX_SONG: usize = 256;
/// The slowest tempo, in beats per minute.
pub const MIN_BPM: f32 = 40.0;
/// The fastest tempo, in beats per minute.
pub const MAX_BPM: f32 = 240.0;
/// The largest swing amount: an odd step sounds half a step late.
pub const MAX_SWING: f32 = 0.5;

/// How many commands fit in the queue from the engine to the audio callback. Commands beyond this
/// wait in the engine's backlog, except drum hits, which are dropped.
pub const QUEUE_CAPACITY: usize = 4096;

/// The tempo of a new engine, in beats per minute.
const DEFAULT_BPM: f32 = 120.0;
/// The length of every pattern of a new engine: one bar.
const DEFAULT_LENGTH: usize = 16;
/// Steps per beat. The grid is fixed at 16th notes in 4/4.
const STEPS_PER_BEAT: f64 = 4.0;
/// Steps per bar.
const BAR: usize = 16;

/// The MIDI channel index of General MIDI drums (channel 10).
const DRUM_CHANNEL: i32 = Synthesizer::PERCUSSION_CHANNEL as i32;

/// The status byte of a MIDI note-on on the drum channel.
const MIDI_NOTE_ON: u8 = 0x90 | DRUM_CHANNEL as u8;
/// The status byte of a MIDI note-off on the drum channel.
const MIDI_NOTE_OFF: u8 = 0x80 | DRUM_CHANNEL as u8;
/// The status byte of a MIDI program change on the drum channel.
const MIDI_PROGRAM_CHANGE: u8 = 0xC0 | DRUM_CHANNEL as u8;
/// The status byte of a MIDI control change on the drum channel.
const MIDI_CONTROL_CHANGE: u8 = 0xB0 | DRUM_CHANNEL as u8;
/// Controller 7, "Channel volume". General MIDI devices scale the whole channel by it.
const MIDI_CHANNEL_VOLUME: u8 = 7;
/// The channel volume MIDI out sends until [`Engine::set_midi_volume`] says otherwise: full.
pub const DEFAULT_MIDI_VOLUME: u8 = 127;

/// Marks a key that has no note-off pending.
const NO_NOTE_OFF: u64 = u64::MAX;

/// Marks a row that has no drum key yet.
const NO_KEY: u8 = u8::MAX;

/// The pattern number the sequencer uses internally for the preview track. It is never published
/// as a playhead.
const PREVIEW_SLOT: usize = MAX_PATTERNS;

/// The number of General MIDI keys a preview step can hold, one velocity each.
const KEYS: usize = 128;

/// Marks "no playhead" in the playhead atomic.
const NO_PLAYHEAD: u64 = u64::MAX;

/// Marks "not playing the song" in the song index field of the playhead atomic.
const NO_SONG_INDEX: u64 = 0xFFFF;

/// How many recent step starts the renderer keeps to look back over the output latency, and for
/// placing hits from the MIDI input. At the fastest tempo and 48 kHz this reaches back about
/// 0.8 s, far more than any output latency.
const HISTORY: usize = 32;

/// The longest output latency the renderer believes from the stream timestamps.
const MAX_PLAUSIBLE_LATENCY_SECONDS: f64 = 1.0;

/// What can go wrong loading a bank or opening the audio device.
#[derive(Debug, thiserror::Error)]
pub enum AudioError {
    /// The SoundFont file could not be read or parsed.
    #[error("could not load the SoundFont: {0}")]
    SoundFont(String),
    /// No output device, or the device refused the stream.
    #[error("could not open the audio output: {0}")]
    Device(String),
    /// An offline render was asked for an empty song, or for no steps at all.
    #[error("There is nothing to play.")]
    Nothing,
    /// An offline render was cancelled before it finished.
    #[error("The render was cancelled.")]
    Cancelled,
}

/// Reads and parses a SoundFont file.
pub fn load_soundfont(path: &Path) -> Result<Arc<SoundFont>, AudioError> {
    let file =
        File::open(path).map_err(|e| AudioError::SoundFont(format!("{}: {e}", path.display())))?;
    let mut reader = BufReader::new(file);
    let soundfont = SoundFont::new(&mut reader)
        .map_err(|e| AudioError::SoundFont(format!("{}: {e:?}", path.display())))?;
    let warnings = soundfont.get_warning_count();
    if warnings > 0 {
        log::warn!(
            "{}: {warnings} defective SoundFont records were skipped",
            path.display()
        );
    }
    Ok(Arc::new(soundfont))
}

/// A drum kit: a preset of the percussion bank.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Kit {
    /// The program number to pass to [`Engine::set_kit`].
    pub program: u8,
    /// The preset name, without surrounding spaces.
    pub name: String,
}

/// The rustysynth bank number of percussion presets.
const PERCUSSION_BANK: i32 = 128;

/// Lists every preset of the percussion bank (bank 128), sorted by program, with names trimmed.
/// Where two presets share a program, the first one in the file is kept.
pub fn drum_kits(soundfont: &SoundFont) -> Vec<Kit> {
    let mut kits: Vec<Kit> = Vec::new();
    for preset in soundfont.get_presets() {
        if preset.get_bank_number() != PERCUSSION_BANK {
            continue;
        }
        let Ok(program) = u8::try_from(preset.get_patch_number()) else {
            continue;
        };
        if program > 127 || kits.iter().any(|kit| kit.program == program) {
            continue;
        }
        kits.push(Kit {
            program,
            name: preset.get_name().trim().to_string(),
        });
    }
    kits.sort_by_key(|kit| kit.program);
    kits
}

/// The kit to play for a project that asks for kit `program`, named `name` where the file
/// recorded it, from `kits` (as [`drum_kits`] lists them). In this order: the kit with the same
/// program; else the kit with the same name, ignoring case and whitespace; else the first kit of
/// the program's General Standard group of 8, which is program `program & !7` (a GS synthesizer
/// falls back to that kit too); else Standard, program 0; else the first kit in the list.
/// Returns `None` only if `kits` is empty.
pub fn resolve_kit(kits: &[Kit], program: u8, name: Option<&str>) -> Option<u8> {
    let with_program = |p: u8| kits.iter().find(|kit| kit.program == p);
    if let Some(kit) = with_program(program) {
        return Some(kit.program);
    }
    if let Some(wanted) = name.map(kit_name_key).filter(|key| !key.is_empty())
        && let Some(kit) = kits.iter().find(|kit| kit_name_key(&kit.name) == wanted)
    {
        return Some(kit.program);
    }
    with_program(program & !7)
        .or_else(|| with_program(0))
        .or_else(|| kits.first())
        .map(|kit| kit.program)
}

/// A kit name reduced for comparison: lower case, with every whitespace character removed.
fn kit_name_key(name: &str) -> String {
    name.chars()
        .filter(|c| !c.is_whitespace())
        .flat_map(char::to_lowercase)
        .collect()
}

/// Which of the 128 MIDI keys the percussion preset 128:`program` has a sound for. A key counts
/// when a preset region's key range and one of its instrument's region key ranges both include
/// it, which is how the synthesizer picks the regions a note-on plays. Velocity ranges are not
/// looked at: a key counts if any velocity sounds. A region that sets no key range covers every
/// key. Where the bank holds two presets 128:`program`, the last one counts, as in the
/// synthesizer. All false if the SoundFont has no such preset; this does not fall back to
/// Standard as the synthesizer does.
pub fn kit_keys(soundfont: &SoundFont, program: u8) -> [bool; 128] {
    let mut keys = [false; 128];
    let Some(preset) = soundfont.get_presets().iter().rev().find(|preset| {
        preset.get_bank_number() == PERCUSSION_BANK
            && preset.get_patch_number() == i32::from(program)
    }) else {
        return keys;
    };
    let instruments = soundfont.get_instruments();
    for preset_region in preset.get_regions() {
        let Some(instrument) = instruments.get(preset_region.get_instrument_id()) else {
            continue;
        };
        let preset_range = preset_region.get_key_range_start()..=preset_region.get_key_range_end();
        for region in instrument.get_regions() {
            let start = region.get_key_range_start().max(*preset_range.start());
            let end = region.get_key_range_end().min(*preset_range.end());
            for key in start.max(0)..=end.min(127) {
                keys[key as usize] = true;
            }
        }
    }
    keys
}

/// A position of the sequencer:which pattern and step sound, and in song mode which entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Playhead {
    /// The pattern slot, below [`MAX_PATTERNS`].
    pub pattern: usize,
    /// The step within that pattern, below its length.
    pub step: usize,
    /// In song mode, the index of the song entry; `None` in pattern mode.
    pub song_index: Option<usize>,
}

impl Playhead {
    /// Packs the playhead into one `u64` for the playhead atomic: the song index in bits 32 to
    /// 47, the pattern in bits 16 to 31 and the step in bits 0 to 15.
    fn pack(this: Option<Playhead>) -> u64 {
        match this {
            None => NO_PLAYHEAD,
            Some(p) => {
                let index = p.song_index.map_or(NO_SONG_INDEX, |i| i as u64);
                (index << 32) | ((p.pattern as u64) << 16) | p.step as u64
            }
        }
    }

    /// The reverse of [`Playhead::pack`].
    fn unpack(packed: u64) -> Option<Playhead> {
        if packed == NO_PLAYHEAD {
            return None;
        }
        let index = (packed >> 32) & 0xFFFF;
        Some(Playhead {
            pattern: ((packed >> 16) & 0xFFFF) as usize,
            step: (packed & 0xFFFF) as usize,
            song_index: (index != NO_SONG_INDEX).then_some(index as usize),
        })
    }
}

/// One hit of a preview, for [`Engine::preview`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreviewHit {
    /// The step from the start of the preview, 16 to a bar.
    pub step: usize,
    /// The General MIDI drum key, 0..=127.
    pub key: u8,
    /// The velocity, 1..=127, played as it is.
    pub velocity: u8,
}

/// A message from the engine to the audio callback.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Command {
    NoteOn {
        key: u8,
        velocity: u8,
    },
    ProgramChange {
        program: u8,
    },
    SetRowKey {
        row: u8,
        key: u8,
    },
    SetStep {
        pattern: u8,
        row: u8,
        step: u8,
        velocity: u8,
    },
    /// The length of a pattern in steps, 1..=`MAX_STEPS`.
    SetLength {
        pattern: u8,
        steps: u16,
    },
    /// Turns off every step of every row of a pattern.
    ClearPattern {
        pattern: u8,
    },
    /// The gain of a row, 0.0..=1.0.
    SetRowGain {
        row: u8,
        gain: f32,
    },
    SetTempo {
        bpm: f32,
    },
    /// The swing amount, 0.0..=`MAX_SWING`.
    SetSwing {
        amount: f32,
    },
    /// One entry of the song order. The song length is sent after the entries.
    SetSongEntry {
        index: u8,
        pattern: u8,
    },
    /// The number of song entries, 0..=`MAX_SONG`.
    SetSongLength {
        entries: u16,
    },
    SetSongLoop {
        looped: bool,
    },
    PlayPattern {
        pattern: u8,
        /// The step to start at; a step past the pattern's end starts at step 0.
        from: u16,
    },
    CuePattern {
        pattern: u8,
    },
    /// The bar that pattern mode loops over, or `None` for the whole pattern.
    SetLoopBar {
        bar: Option<u8>,
    },
    PlaySong {
        from: u8,
    },
    Stop,
    /// Turns MIDI out on or off.
    SetMidiOut {
        on: bool,
    },
    /// The channel volume MIDI out sends, 0..=127.
    SetMidiVolume {
        volume: u8,
    },
    /// The gain of the whole output, 0.0..=1.0.
    SetVolume {
        gain: f32,
    },
    /// Stops the preview if it plays, and clears all its hits.
    ClearPreview,
    /// One hit of the preview. Of two hits on the same step and key, the louder one counts.
    SetPreviewHit {
        step: u8,
        key: u8,
        velocity: u8,
    },
    /// Loops the preview over its first `steps` steps, 1..=`MAX_STEPS`, from step 0, starting
    /// now. Stops the project's play.
    PlayPreview {
        steps: u16,
    },
    /// Stops the preview if it plays. Leaves the project's play alone.
    StopPreview,
    /// Fades out every sound still ringing over [`SILENCE_SECONDS`], and ends the notes MIDI
    /// out still holds. Leaves play alone.
    Silence,
}

impl Command {
    /// The command that sets the key of `row`, or `None` if `row` is out of range.
    fn row_key(row: usize, key: u8) -> Option<Command> {
        (row < MAX_ROWS).then(|| Command::SetRowKey {
            row: row as u8,
            key: key.min(127),
        })
    }

    /// The command that sets the gain of `row`, or `None` if `row` is out of range or the gain
    /// is not a number.
    fn row_gain(row: usize, gain: f32) -> Option<Command> {
        (row < MAX_ROWS && !gain.is_nan()).then(|| Command::SetRowGain {
            row: row as u8,
            gain: gain.clamp(0.0, 1.0),
        })
    }

    /// The command that clears `pattern`, or `None` if it is out of range.
    fn clear_pattern(pattern: usize) -> Option<Command> {
        (pattern < MAX_PATTERNS).then_some(Command::ClearPattern {
            pattern: pattern as u8,
        })
    }

    /// The command that sets one step, or `None` if a position is out of range.
    fn step(pattern: usize, row: usize, step: usize, velocity: u8) -> Option<Command> {
        (pattern < MAX_PATTERNS && row < MAX_ROWS && step < MAX_STEPS).then(|| Command::SetStep {
            pattern: pattern as u8,
            row: row as u8,
            step: step as u8,
            velocity: velocity.min(127),
        })
    }

    /// The command that sets the length of `pattern`, or `None` if it is out of range.
    fn length(pattern: usize, steps: usize) -> Option<Command> {
        (pattern < MAX_PATTERNS).then(|| Command::SetLength {
            pattern: pattern as u8,
            steps: steps.clamp(1, MAX_STEPS) as u16,
        })
    }

    /// The command that sets the tempo, or `None` if `bpm` is not a number.
    fn tempo(bpm: f32) -> Option<Command> {
        (!bpm.is_nan()).then(|| Command::SetTempo {
            bpm: bpm.clamp(MIN_BPM, MAX_BPM),
        })
    }

    /// The command that sets the swing, or `None` if `amount` is not a number.
    fn swing(amount: f32) -> Option<Command> {
        (!amount.is_nan()).then(|| Command::SetSwing {
            amount: amount.clamp(0.0, MAX_SWING),
        })
    }

    /// The commands that set the whole song order: one per entry, then the length. Entries
    /// beyond `MAX_SONG` are dropped, and a slot at or above `MAX_PATTERNS` becomes the last
    /// slot.
    fn song(order: &[usize]) -> impl Iterator<Item = Command> + '_ {
        let order = &order[..order.len().min(MAX_SONG)];
        let entries = order
            .iter()
            .enumerate()
            .map(|(index, &pattern)| Command::SetSongEntry {
                index: index as u8,
                pattern: pattern.min(MAX_PATTERNS - 1) as u8,
            });
        entries.chain(std::iter::once(Command::SetSongLength {
            entries: order.len() as u16,
        }))
    }

    /// The commands that replace the preview. The first stops and clears the old one. Then comes
    /// one per hit, and last one that starts the new preview over `steps` steps, clamped to
    /// 1..=`MAX_STEPS`. A hit at or after that step, with a key above 127 or with velocity 0 is
    /// left out. A velocity above 127 plays at 127.
    fn preview(hits: &[PreviewHit], steps: usize) -> impl Iterator<Item = Command> + '_ {
        let steps = steps.clamp(1, MAX_STEPS);
        let hits = hits
            .iter()
            .filter(move |hit| hit.step < steps && usize::from(hit.key) < KEYS && hit.velocity > 0)
            .map(|hit| Command::SetPreviewHit {
                step: hit.step as u8,
                key: hit.key,
                velocity: hit.velocity.min(127),
            });
        std::iter::once(Command::ClearPreview)
            .chain(hits)
            .chain(std::iter::once(Command::PlayPreview {
                steps: steps as u16,
            }))
    }

    /// Whether the command starts or stops play. The engine and the renderer both count these,
    /// so the engine knows when the renderer has applied the last one.
    fn is_transport(&self) -> bool {
        matches!(
            self,
            Command::PlayPattern { .. }
                | Command::PlaySong { .. }
                | Command::Stop
                | Command::PlayPreview { .. }
                | Command::StopPreview
        )
    }
}

/// The sending end of the command queue. It never blocks.
struct Commands {
    producer: rtrb::Producer<Command>,
    /// Commands that did not fit in the queue, oldest first. They must not be lost, so they are
    /// sent before any newer command.
    backlog: VecDeque<Command>,
}

/// Makes the command queue: the sending end for the engine and the receiving end for the
/// renderer.
fn command_queue() -> (Commands, rtrb::Consumer<Command>) {
    let (producer, consumer) = rtrb::RingBuffer::new(QUEUE_CAPACITY);
    let commands = Commands {
        producer,
        backlog: VecDeque::new(),
    };
    (commands, consumer)
}

impl Commands {
    /// Sends a drum hit, or drops it if the queue is full.
    fn hit(&mut self, key: u8, velocity: u8) {
        self.flush();
        let command = Command::NoteOn {
            key: key.min(127),
            velocity: velocity.clamp(1, 127),
        };
        if self.producer.push(command).is_err() {
            log::debug!("command queue full, dropped a hit on key {key}");
        }
    }

    /// Sends a command that must arrive. If the queue is full, it waits in the backlog behind any
    /// older command that is waiting there.
    fn send(&mut self, command: Command) {
        if !self.flush() || self.producer.push(command).is_err() {
            if self.backlog.is_empty() {
                log::warn!(
                    "command queue full; holding commands until the audio thread catches up"
                );
            }
            self.backlog.push_back(command);
        }
    }

    /// Moves backlog commands into the queue while there is room. Returns whether the backlog is
    /// now empty.
    fn flush(&mut self) -> bool {
        while let Some(&command) = self.backlog.front() {
            if self.producer.push(command).is_err() {
                return false;
            }
            self.backlog.pop_front();
        }
        true
    }

    fn set_kit(&mut self, program: u8) {
        self.send(Command::ProgramChange {
            program: program.min(127),
        });
    }

    /// Sends the whole song order: one command per entry, then the length. Entries beyond
    /// `MAX_SONG` are dropped, and a slot at or above `MAX_PATTERNS` becomes the last slot.
    /// Returns the number of entries sent.
    fn set_song(&mut self, order: &[usize]) -> usize {
        for command in Command::song(order) {
            self.send(command);
        }
        order.len().min(MAX_SONG)
    }

    /// Sends `command` if there is one.
    fn send_some(&mut self, command: Option<Command>) {
        if let Some(command) = command {
            self.send(command);
        }
    }
}

/// The step timing of the sequencer, without the patterns or the synthesizer.
///
/// Frames are counted from the start of rendering. The clock knows when the next step is due but
/// not which step it is; the sequencer works that out.
#[derive(Debug, Clone)]
struct Clock {
    sample_rate: f64,
    frames_per_step: f64,
    playing: bool,
    /// The grid time of a step that fired or will fire, in frames. Steps after it are due at
    /// whole multiples of `frames_per_step` later, which keeps the timing free of drift.
    anchor: f64,
    /// How many steps after `anchor` the next step is.
    steps_since_anchor: u64,
    /// The swing amount, 0.0..=`MAX_SWING`: how far each odd step sounds after its grid time,
    /// as a fraction of a step.
    swing: f64,
}

impl Clock {
    fn new(sample_rate: u32) -> Self {
        let sample_rate = f64::from(sample_rate);
        Clock {
            sample_rate,
            frames_per_step: frames_per_step(sample_rate, DEFAULT_BPM),
            playing: false,
            anchor: 0.0,
            steps_since_anchor: 0,
            swing: 0.0,
        }
    }

    /// The unswung grid time of the next step, in frames.
    fn next_grid_frame(&self) -> f64 {
        self.anchor + self.steps_since_anchor as f64 * self.frames_per_step
    }

    /// How far `step` sounds after its grid time, in frames: the swing delay for an odd step,
    /// zero for an even one. The grid itself never moves, so swing cannot cause drift.
    fn step_offset(&self, step: usize) -> f64 {
        if step % 2 == 1 {
            self.swing * self.frames_per_step
        } else {
            0.0
        }
    }

    /// Sets the swing amount, clamped to 0.0..=`MAX_SWING`; a value that is not a number counts
    /// as 0. It applies to the next step that fires, so a step whose straight time has passed
    /// but whose swung time has not may fire in the current block, or later, but always once.
    fn set_swing(&mut self, amount: f32) {
        self.swing = if amount.is_nan() {
            0.0
        } else {
            f64::from(amount.clamp(0.0, MAX_SWING))
        };
    }

    /// The frame at which the next step is due, if it is `step` of its pattern.
    fn due(&self, step: usize) -> f64 {
        self.next_grid_frame() + self.step_offset(step)
    }

    /// Starts the grid at frame `now`, also if already playing.
    fn play(&mut self, now: u64) {
        self.playing = true;
        self.anchor = now as f64;
        self.steps_since_anchor = 0;
    }

    fn stop(&mut self) {
        self.playing = false;
    }

    /// Moves the grid on by one step, after a step fired.
    fn tick(&mut self) {
        self.steps_since_anchor += 1;
    }

    /// Changes the tempo at frame `now`. While playing, the part of the current step still to
    /// come is stretched or shrunk in proportion, so the position within the step is kept.
    fn set_tempo(&mut self, bpm: f32, now: u64) {
        let new = frames_per_step(self.sample_rate, bpm);
        if self.playing {
            let remaining = (self.next_grid_frame() - now as f64).max(0.0);
            self.anchor = now as f64 + remaining * new / self.frames_per_step;
            self.steps_since_anchor = 0;
        }
        self.frames_per_step = new;
    }
}

/// The length of a step in frames at `bpm`, which is clamped to the allowed range.
fn frames_per_step(sample_rate: f64, bpm: f32) -> f64 {
    sample_rate * 60.0 / (f64::from(bpm.clamp(MIN_BPM, MAX_BPM)) * STEPS_PER_BEAT)
}

/// One recent step start, or the frame where play stopped.
#[derive(Debug, Clone, Copy)]
struct StepStart {
    /// The first frame of the block the step fired in, or the frame where play stopped.
    frame: u64,
    /// The step's straight grid time in frames, without swing. Where play stopped, `frame`.
    grid: f64,
    /// The length of a step in frames when the step fired.
    frames_per_step: f64,
    /// The position that started, or `None` where play stopped or a preview step started.
    position: Option<Playhead>,
}

/// The frames at which recent steps started, newest last, for looking back over the output
/// latency.
#[derive(Debug, Clone)]
struct StepHistory {
    /// The step starts in a ring.
    entries: [StepStart; HISTORY],
    /// The index of the newest entry.
    newest: usize,
}

impl StepHistory {
    fn new() -> Self {
        StepHistory {
            entries: [StepStart {
                frame: 0,
                grid: 0.0,
                frames_per_step: 1.0,
                position: None,
            }; HISTORY],
            newest: 0,
        }
    }

    fn record(&mut self, start: StepStart) {
        self.newest = (self.newest + 1) % HISTORY;
        self.entries[self.newest] = start;
    }

    /// The entries, newest first.
    fn newest_first(&self) -> impl Iterator<Item = &StepStart> {
        (0..HISTORY).map(|back| &self.entries[(self.newest + HISTORY - back) % HISTORY])
    }

    /// The position sounding at `frame`: the newest one that started at or before it. `None` if
    /// play was stopped then, or if `frame` is older than the whole history.
    fn at(&self, frame: u64) -> Option<Playhead> {
        self.newest_first()
            .find(|start| start.frame <= frame)
            .and_then(|start| start.position)
    }

    /// The position at `frame` in straight time: the newest step whose grid time is at or before
    /// `frame`, and how many steps have passed since that grid time. `None` if play was stopped
    /// then, or if `frame` is older than the whole history.
    fn straight_at(&self, frame: f64) -> Option<(Playhead, f64)> {
        let start = self.newest_first().find(|start| start.grid <= frame)?;
        let position = start.position?;
        Some((position, (frame - start.grid) / start.frames_per_step))
    }
}

/// Whether the sequencer loops one pattern, plays the song or loops the preview.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Pattern,
    Song,
    /// The preview track. Its positions use the pattern number `PREVIEW_SLOT`.
    Preview,
}

/// The velocities of one pattern: `[row][step]`.
type PatternSteps = [[u8; MAX_STEPS]; MAX_ROWS];

/// `gain` clamped to 0.0..=1.0, with NaN as 1.0.
fn clamp_gain(gain: f32) -> f32 {
    if gain.is_nan() {
        1.0
    } else {
        gain.clamp(0.0, 1.0)
    }
}

/// The velocity a step of `velocity` plays at in a row of `gain`, clamped to 0.0..=1.0. Zero
/// means the note is not played. The crate documentation explains why the scaling is linear.
fn scaled_velocity(velocity: u8, gain: f32) -> u8 {
    let gain = if gain.is_nan() {
        0.0
    } else {
        gain.clamp(0.0, 1.0)
    };
    (f32::from(velocity) * gain).round() as u8
}

/// The patterns, the song and the clock: everything the audio thread needs to know which drums to
/// start when, but not the synthesizer.
struct Sequencer {
    clock: Clock,
    /// The General MIDI drum key of each row, or `NO_KEY`. Shared by all patterns.
    keys: [u8; MAX_ROWS],
    /// The gain of each row, 0.0..=1.0. Shared by all patterns.
    gains: [f32; MAX_ROWS],
    /// `MAX_PATTERNS` patterns, allocated once in `new`.
    patterns: Box<[PatternSteps]>,
    /// The length of each pattern in steps, 1..=`MAX_STEPS`.
    lengths: [usize; MAX_PATTERNS],
    /// The pattern slot of each song entry; the first `song_len` are in use.
    song: [u8; MAX_SONG],
    song_len: usize,
    song_loop: bool,
    mode: Mode,
    /// The position play starts at, until its step fires.
    first: Option<Playhead>,
    /// The position that fired last, while playing.
    last: Option<Playhead>,
    /// In pattern mode, the pattern to switch to at the next bar boundary.
    cue: Option<usize>,
    /// The bar that pattern mode loops over, or `None` for the whole pattern. A bar at or past
    /// the playing pattern's bar count counts as `None`.
    loop_bar: Option<usize>,
    history: StepHistory,
    /// The preview track: the velocity of each key at each of `MAX_STEPS` steps, 0 for none.
    /// Allocated once in `new`.
    preview: Box<[[u8; KEYS]]>,
    /// The number of steps the preview loops over, 1..=`MAX_STEPS`.
    preview_len: usize,
}

impl Sequencer {
    fn new(sample_rate: u32) -> Self {
        Sequencer {
            clock: Clock::new(sample_rate),
            keys: [NO_KEY; MAX_ROWS],
            gains: [1.0; MAX_ROWS],
            patterns: vec![[[0; MAX_STEPS]; MAX_ROWS]; MAX_PATTERNS].into_boxed_slice(),
            lengths: [DEFAULT_LENGTH; MAX_PATTERNS],
            song: [0; MAX_SONG],
            song_len: 0,
            song_loop: false,
            mode: Mode::Pattern,
            first: None,
            last: None,
            cue: None,
            loop_bar: None,
            history: StepHistory::new(),
            preview: vec![[0; KEYS]; MAX_STEPS].into_boxed_slice(),
            preview_len: BAR,
        }
    }

    /// Whether a project pattern or the song plays, as opposed to the preview or nothing.
    fn plays_project(&self) -> bool {
        self.clock.playing && self.mode != Mode::Preview
    }

    /// The steps of `pattern` that the loop bar covers, as `start..end`, or `None` if no loop bar
    /// is set or the pattern is too short to have that bar. The end is the pattern's length if
    /// the pattern ends inside the bar. Applies to pattern mode only; the caller checks the mode.
    fn loop_span(&self, pattern: usize) -> Option<std::ops::Range<usize>> {
        let length = *self.lengths.get(pattern)?;
        let start = self.loop_bar? * BAR;
        (start < length).then(|| start..(start + BAR).min(length))
    }

    /// `step` of `pattern` moved into the loop bar, keeping its offset within its bar, wrapped
    /// to the loop bar's length. A step already inside the loop bar, or any step when no loop
    /// bar applies, stays as it is.
    fn moved_into_loop(&self, pattern: usize, step: usize) -> usize {
        match self.loop_span(pattern) {
            Some(span) if !span.contains(&step) => span.start + step % BAR % span.len(),
            _ => step,
        }
    }

    /// Whether the preview plays.
    fn plays_preview(&self) -> bool {
        self.clock.playing && self.mode == Mode::Preview
    }

    /// The velocity a live hit on `key` plays at: scaled by the gain of the first row with that
    /// key, as a sequenced note of that row would be, or as it is if no row has the key. Zero
    /// means the hit is not played.
    fn live_velocity(&self, key: u8, velocity: u8) -> u8 {
        match self.keys.iter().position(|&row_key| row_key == key) {
            Some(row) => scaled_velocity(velocity, self.gains[row]),
            None => velocity,
        }
    }

    /// Where the project was at `frame`, in straight time, or `None` if it was not playing then
    /// (stopped, or the preview played) or `frame` is older than the step history.
    ///
    /// The step is the newest one whose straight grid time is at or before `frame`, plus the
    /// steps since that grid time. The next step counts too, before it fires: with swing, an odd
    /// step fires after its grid time, and a hit may come from a little after the newest step
    /// known. A result at or past the pattern's length wraps to its start.
    fn heard_position(&self, frame: f64) -> Option<HeardPosition> {
        let pending = if self.plays_project() {
            self.next_position()
                .map(|position| (position, self.clock.next_grid_frame()))
                .filter(|&(_, grid)| grid <= frame)
        } else {
            None
        };
        let (position, steps) = match pending {
            Some((position, grid)) => (position, (frame - grid) / self.clock.frames_per_step),
            None => self.history.straight_at(frame)?,
        };
        let length = *self.lengths.get(position.pattern)? as f64;
        let mut step = position.step as f64 + steps;
        let span = self
            .loop_span(position.pattern)
            .filter(|span| self.mode == Mode::Pattern && span.contains(&position.step));
        if let Some(span) = span {
            let (start, end) = (span.start as f64, span.end as f64);
            if step >= end {
                step = start + (step - start) % (end - start);
            }
        } else if step >= length {
            step %= length;
        }
        Some(HeardPosition {
            pattern: position.pattern,
            song_index: position.song_index,
            step,
        })
    }

    /// Applies a sequencer command at frame `now`. Other commands are ignored.
    fn apply(&mut self, command: Command, now: u64) {
        match command {
            Command::SetRowKey { row, key } => {
                if let Some(slot) = self.keys.get_mut(usize::from(row)) {
                    *slot = key;
                }
            }
            Command::SetStep {
                pattern,
                row,
                step,
                velocity,
            } => {
                if let Some(slot) = self
                    .patterns
                    .get_mut(usize::from(pattern))
                    .and_then(|pattern| pattern.get_mut(usize::from(row)))
                    .and_then(|row| row.get_mut(usize::from(step)))
                {
                    *slot = velocity;
                }
            }
            Command::SetLength { pattern, steps } => {
                if let Some(slot) = self.lengths.get_mut(usize::from(pattern)) {
                    *slot = usize::from(steps).clamp(1, MAX_STEPS);
                }
            }
            Command::ClearPattern { pattern } => {
                if let Some(pattern) = self.patterns.get_mut(usize::from(pattern)) {
                    for row in pattern.iter_mut() {
                        row.fill(0);
                    }
                }
            }
            Command::SetRowGain { row, gain } => {
                if let Some(slot) = self.gains.get_mut(usize::from(row)) {
                    *slot = gain;
                }
            }
            Command::SetTempo { bpm } => self.clock.set_tempo(bpm, now),
            Command::SetSwing { amount } => self.clock.set_swing(amount),
            Command::SetSongEntry { index, pattern } => {
                self.song[usize::from(index)] = pattern.min(MAX_PATTERNS as u8 - 1);
            }
            Command::SetSongLength { entries } => {
                self.song_len = usize::from(entries).min(MAX_SONG);
            }
            Command::SetSongLoop { looped } => self.song_loop = looped,
            Command::PlayPattern { pattern, from } => {
                let pattern = usize::from(pattern);
                if pattern < MAX_PATTERNS {
                    let from = usize::from(from);
                    let from = if from < self.lengths[pattern] {
                        from
                    } else {
                        0
                    };
                    self.start(
                        Mode::Pattern,
                        Playhead {
                            pattern,
                            step: self.moved_into_loop(pattern, from),
                            song_index: None,
                        },
                        now,
                    );
                }
            }
            Command::CuePattern { pattern } => {
                let pattern = usize::from(pattern);
                let current = self.first.or(self.last).map(|p| p.pattern);
                if self.clock.playing && self.mode == Mode::Pattern && pattern < MAX_PATTERNS {
                    self.cue = (current != Some(pattern)).then_some(pattern);
                }
            }
            Command::SetLoopBar { bar } => {
                self.loop_bar = bar.map(usize::from);
                if let Some(first) = self.first.filter(|_| self.mode == Mode::Pattern) {
                    self.first = Some(Playhead {
                        step: self.moved_into_loop(first.pattern, first.step),
                        ..first
                    });
                }
            }
            Command::PlaySong { from } => {
                let index = usize::from(from);
                if index < self.song_len {
                    self.loop_bar = None;
                    self.start(
                        Mode::Song,
                        Playhead {
                            pattern: usize::from(self.song[index]),
                            step: 0,
                            song_index: Some(index),
                        },
                        now,
                    );
                }
            }
            Command::Stop => {
                if self.clock.playing {
                    self.halt(now);
                }
            }
            Command::ClearPreview => {
                if self.plays_preview() {
                    self.halt(now);
                }
                for step in self.preview.iter_mut() {
                    step.fill(0);
                }
            }
            Command::SetPreviewHit {
                step,
                key,
                velocity,
            } => {
                if let Some(slot) = self
                    .preview
                    .get_mut(usize::from(step))
                    .and_then(|step| step.get_mut(usize::from(key)))
                {
                    *slot = (*slot).max(velocity.min(127));
                }
            }
            Command::PlayPreview { steps } => {
                self.preview_len = usize::from(steps).clamp(1, MAX_STEPS);
                self.start(
                    Mode::Preview,
                    Playhead {
                        pattern: PREVIEW_SLOT,
                        step: 0,
                        song_index: None,
                    },
                    now,
                );
            }
            Command::StopPreview => {
                if self.plays_preview() {
                    self.halt(now);
                }
            }
            Command::NoteOn { .. }
            | Command::ProgramChange { .. }
            | Command::SetMidiOut { .. }
            | Command::SetMidiVolume { .. }
            | Command::SetVolume { .. }
            | Command::Silence => {
                // The renderer applies these; they do not touch the sequencer.
            }
        }
    }

    /// Starts play at `first` in `mode`, with the first step due at frame `now`.
    fn start(&mut self, mode: Mode, first: Playhead, now: u64) {
        self.mode = mode;
        self.first = Some(first);
        self.last = None;
        self.cue = None;
        self.clock.play(now);
    }

    /// Stops play at frame `frame`.
    fn halt(&mut self, frame: u64) {
        self.clock.stop();
        self.first = None;
        self.last = None;
        self.cue = None;
        self.history.record(StepStart {
            frame,
            grid: frame as f64,
            frames_per_step: self.clock.frames_per_step,
            position: None,
        });
    }

    /// The position of the next step, or `None` if a song that does not loop has ended.
    fn next_position(&self) -> Option<Playhead> {
        if self.first.is_some() {
            return self.first;
        }
        let last = self.last?;
        let next = last.step + 1;
        if self.mode == Mode::Preview {
            return Some(Playhead {
                step: next % self.preview_len,
                ..last
            });
        }
        let length = self.lengths[last.pattern];
        match self.mode {
            Mode::Pattern => {
                let boundary = next % BAR == 0 || next >= length;
                if let Some(cue) = self.cue.filter(|&cue| boundary && cue != last.pattern) {
                    let step = match self.loop_span(cue) {
                        Some(span) => span.start,
                        None => {
                            let bars = self.lengths[cue].div_ceil(BAR);
                            (last.step / BAR + 1) % bars * BAR
                        }
                    };
                    return Some(Playhead {
                        pattern: cue,
                        step,
                        song_index: None,
                    });
                }
                let step = match self.loop_span(last.pattern) {
                    Some(span) if span.contains(&last.step) && next >= span.end => span.start,
                    _ => self.moved_into_loop(last.pattern, next % length),
                };
                Some(Playhead { step, ..last })
            }
            Mode::Song => {
                if next < length {
                    return Some(Playhead { step: next, ..last });
                }
                let index = last.song_index? + 1;
                let index = if index < self.song_len {
                    index
                } else if self.song_loop && self.song_len > 0 {
                    0
                } else {
                    return None;
                };
                Some(Playhead {
                    pattern: usize::from(self.song[index]),
                    step: 0,
                    song_index: Some(index),
                })
            }
            Mode::Preview => unreachable!("the preview returned above"),
        }
    }

    /// Starts the steps due in the block of `frames` frames at `start`. For each step, calls
    /// `on_step` with its position, then `note` with the key and velocity of every row that has a
    /// key and an active step, with the velocity scaled by the row's gain. A row whose scaled
    /// velocity is 0 plays nothing. Stops play when a song that does not loop reaches the frame where
    /// the step after its last would have started.
    ///
    /// In preview mode the position's pattern is `PREVIEW_SLOT`, and `note` gets every key of the
    /// preview step with its velocity as it is, without rows or gains. The step history records
    /// no position for a preview step, so the playhead shows nothing.
    fn advance(
        &mut self,
        start: u64,
        frames: u64,
        mut on_step: impl FnMut(Playhead),
        mut note: impl FnMut(u8, u8),
    ) {
        let end = (start + frames) as f64;
        while self.clock.playing {
            let Some(position) = self.next_position() else {
                if self.clock.next_grid_frame() < end {
                    self.halt(start);
                }
                return;
            };
            if self.clock.due(position.step) >= end {
                return;
            }
            self.first = None;
            self.last = Some(position);
            if self.cue == Some(position.pattern) {
                self.cue = None;
            }
            let mut step_start = StepStart {
                frame: start,
                grid: self.clock.next_grid_frame(),
                frames_per_step: self.clock.frames_per_step,
                position: None,
            };
            self.clock.tick();
            if self.mode == Mode::Preview {
                self.history.record(step_start);
                on_step(position);
                for (key, &velocity) in self.preview[position.step].iter().enumerate() {
                    if velocity > 0 {
                        note(key as u8, velocity);
                    }
                }
                continue;
            }
            step_start.position = Some(position);
            self.history.record(step_start);
            on_step(position);
            let steps = &self.patterns[position.pattern];
            for ((key, gain), row) in self.keys.iter().zip(&self.gains).zip(steps.iter()) {
                let velocity = scaled_velocity(row[position.step], *gain);
                if *key != NO_KEY && velocity > 0 {
                    note(*key, velocity);
                }
            }
        }
    }
}

/// What the audio callback publishes for the engine to read without locking.
#[derive(Debug)]
struct Status {
    /// The position the listener hears, packed by [`Playhead::pack`].
    playhead: AtomicU64,
    /// Whether the sequencer is playing a project pattern or the song.
    playing: AtomicBool,
    /// Whether the sequencer is playing the preview.
    previewing: AtomicBool,
    /// How many play and stop commands the callback has applied, wrapping.
    transport: AtomicU32,
    /// The output latency the callback last used, in frames.
    latency_frames: AtomicU32,
}

/// The synthesizer, the sequencer and the receiving end of the command queue, independent of any
/// audio device.
///
/// The audio callback calls [`Renderer::render`]; tests call it directly to render offline.
/// Nothing in `render` allocates, locks or blocks.
pub(crate) struct Renderer {
    synth: Synthesizer,
    consumer: rtrb::Consumer<Command>,
    sequencer: Sequencer,
    status: Arc<Status>,
    /// How many play and stop commands have been applied, wrapping.
    transport: u32,
    /// How many frames pass between `render` writing a frame and the listener hearing it.
    latency_frames: u64,
    left: Vec<f32>,
    right: Vec<f32>,
    /// The next frame of `left` and `right` to output. Equal to the block size when the block
    /// has been used up.
    read: usize,
    /// Frames rendered since the start, counted at block boundaries.
    clock: u64,
    /// For each key, the frame at which the synthesizer's note-off is due, or `NO_NOTE_OFF`.
    note_off_at: [u64; 128],
    ring_frames: u64,
    /// The frames left of a fade started by [`Command::Silence`], or 0 when none runs.
    fade_left: u64,
    /// The length of a whole fade in frames, at least 1.
    fade_frames: u64,
    /// The output gain set by [`Command::SetVolume`], 0.0..=1.0.
    volume: f32,
    /// The output gain at the end of the last block. The next block ramps from it to `volume`.
    volume_reached: f32,
    /// For each key, the frame at which MIDI out's note-off is due, or `NO_NOTE_OFF`. Kept apart
    /// from `note_off_at` because MIDI notes end much sooner; see [`MIDI_NOTE_SECONDS`].
    midi_off_at: [u64; 128],
    midi_note_frames: u64,
    sample_rate: f64,
    /// The sending end of the MIDI queue.
    midi: rtrb::Producer<MidiEvent>,
    /// The receiving end of the MIDI queue, until [`Renderer::take_midi_events`] hands it out.
    midi_events: Option<MidiEvents>,
    /// Whether MIDI out is on. While it is, every note also goes to the MIDI queue and the
    /// synthesizer's output is replaced with silence.
    midi_out: bool,
    /// The last kit program applied. It is sent again when MIDI out is turned on.
    program: u8,
    /// The channel volume for MIDI out, 0..=127. It is sent again when MIDI out is turned on.
    midi_volume: u8,
    /// The moment the current `render` call started, taken the first time a MIDI message needs
    /// it. Only used while MIDI out is on.
    callback_now: Option<Instant>,
    /// How many frames of the current `render` call's buffer were written before the current
    /// block.
    block_offset: usize,
    /// The frame the current `render` call outputs first. The listener hears it one output
    /// latency after `callback_now`.
    call_start: u64,
    /// The receiving end of the live hit queue.
    live: rtrb::Consumer<LiveHit>,
    /// The sending end of the live hit queue, until [`Renderer::take_live_hits`] hands it out.
    live_hits: Option<LiveHits>,
    /// The sending end of the heard hit queue.
    heard: rtrb::Producer<HeardHit>,
    /// The receiving end of the heard hit queue, until [`Renderer::take_heard_hits`] hands it
    /// out.
    heard_hits: Option<rtrb::Consumer<HeardHit>>,
}

impl Renderer {
    /// Builds a synthesizer at `sample_rate` and a command queue feeding it.
    fn new(
        soundfont: &Arc<SoundFont>,
        sample_rate: u32,
    ) -> Result<(Commands, Renderer), AudioError> {
        let rate = i32::try_from(sample_rate)
            .map_err(|_| AudioError::Device(format!("unsupported sample rate {sample_rate}")))?;
        let settings = SynthesizerSettings::new(rate);
        let synth = Synthesizer::new(soundfont, &settings)
            .map_err(|e| AudioError::Device(format!("synthesizer at {sample_rate} Hz: {e:?}")))?;
        let block = synth.get_block_size();
        let (commands, consumer) = command_queue();
        let (midi, midi_events) = rtrb::RingBuffer::new(MIDI_QUEUE_CAPACITY);
        let (live_hits, live) = rtrb::RingBuffer::new(LIVE_QUEUE_CAPACITY);
        let (heard, heard_hits) = rtrb::RingBuffer::new(LIVE_QUEUE_CAPACITY);
        let renderer = Renderer {
            synth,
            consumer,
            sequencer: Sequencer::new(sample_rate),
            status: Arc::new(Status {
                playhead: AtomicU64::new(NO_PLAYHEAD),
                playing: AtomicBool::new(false),
                previewing: AtomicBool::new(false),
                transport: AtomicU32::new(0),
                latency_frames: AtomicU32::new(0),
            }),
            transport: 0,
            latency_frames: 0,
            left: vec![0.0; block],
            right: vec![0.0; block],
            read: block,
            clock: 0,
            note_off_at: [NO_NOTE_OFF; 128],
            ring_frames: (RING_SECONDS * sample_rate as f32) as u64,
            fade_left: 0,
            fade_frames: ((SILENCE_SECONDS * sample_rate as f32) as u64).max(1),
            volume: 1.0,
            volume_reached: 1.0,
            midi_off_at: [NO_NOTE_OFF; 128],
            midi_note_frames: (MIDI_NOTE_SECONDS * sample_rate as f32) as u64,
            sample_rate: f64::from(sample_rate),
            midi,
            midi_events: Some(midi_events),
            midi_out: false,
            program: 0,
            midi_volume: DEFAULT_MIDI_VOLUME,
            callback_now: None,
            block_offset: 0,
            call_start: 0,
            live,
            live_hits: Some(live_hits),
            heard,
            heard_hits: Some(heard_hits),
        };
        Ok((commands, renderer))
    }

    /// Hands out the receiving end of the MIDI queue. The first call returns it; later calls
    /// return `None`. The output takes it before the renderer moves to the audio thread.
    fn take_midi_events(&mut self) -> Option<MidiEvents> {
        self.midi_events.take()
    }

    /// Hands out the sending end of the live hit queue. The first call returns it; later calls
    /// return `None`. The output takes it before the renderer moves to the audio thread.
    fn take_live_hits(&mut self) -> Option<LiveHits> {
        self.live_hits.take()
    }

    /// Hands out the receiving end of the heard hit queue. The first call returns it; later
    /// calls return `None`. The output takes it before the renderer moves to the audio thread.
    fn take_heard_hits(&mut self) -> Option<rtrb::Consumer<HeardHit>> {
        self.heard_hits.take()
    }

    /// Sets the output latency used for the playhead.
    fn set_latency(&mut self, frames: u32) {
        self.latency_frames = u64::from(frames);
        self.status.latency_frames.store(frames, Ordering::Relaxed);
    }

    /// Fills an interleaved buffer of `channels` channels. Left goes to the first channel and
    /// right to the second; a mono buffer gets their average; further channels stay silent.
    /// Then publishes the playhead and the play state.
    pub(crate) fn render<T>(&mut self, out: &mut [T], channels: usize)
    where
        T: Sample + FromSample<f32>,
    {
        if channels == 0 {
            return;
        }
        let block = self.left.len();
        self.callback_now = None;
        self.call_start = self.clock - (block - self.read) as u64;
        for (written, frame) in out.chunks_mut(channels).enumerate() {
            if self.read == block {
                self.block_offset = written;
                self.render_block();
            }
            let left = self.left[self.read];
            let right = self.right[self.read];
            self.read += 1;
            match frame {
                [mono] => *mono = T::from_sample(0.5 * (left + right)),
                [l, r, rest @ ..] => {
                    *l = T::from_sample(left);
                    *r = T::from_sample(right);
                    for sample in rest {
                        *sample = T::EQUILIBRIUM;
                    }
                }
                [] => {}
            }
        }
        self.publish();
    }

    /// Publishes the position sounding at the frame the listener hears now (the next frame to
    /// output, less the output latency), then whether the sequencer plays the project and whether
    /// it plays the preview, then the count of applied play and stop commands. The count is
    /// stored last with release ordering, so an engine that sees the count also sees the play
    /// state that goes with it.
    fn publish(&self) {
        let next_out = self.clock - (self.left.len() - self.read) as u64;
        let heard = next_out.saturating_sub(self.latency_frames);
        let position = self.sequencer.history.at(heard);
        self.status
            .playhead
            .store(Playhead::pack(position), Ordering::Relaxed);
        self.status
            .playing
            .store(self.sequencer.plays_project(), Ordering::Relaxed);
        self.status
            .previewing
            .store(self.sequencer.plays_preview(), Ordering::Relaxed);
        self.status
            .transport
            .store(self.transport, Ordering::Release);
    }

    /// Applies the queued commands, the live hits, the due steps and the due note-offs, then
    /// renders one block.
    fn render_block(&mut self) {
        let now = self.clock;
        while let Ok(command) = self.consumer.pop() {
            self.apply(command);
        }
        self.play_live_hits();
        if self.fade_left > 0 && self.sequencer.clock.playing {
            // Play started again during the fade: end it now, so the new hits are not faded.
            self.cut();
        }
        let block = self.left.len() as u64;
        let at = self.midi_out.then(|| self.block_time());
        let Renderer {
            sequencer,
            synth,
            note_off_at,
            ring_frames,
            midi_off_at,
            midi_note_frames,
            midi,
            ..
        } = self;
        sequencer.advance(
            now,
            block,
            |_| {},
            |key, velocity| {
                synth.note_on(DRUM_CHANNEL, i32::from(key), i32::from(velocity));
                note_off_at[usize::from(key.min(127))] = now + *ring_frames;
                if let Some(at) = at {
                    midi_note_on(
                        midi,
                        midi_off_at,
                        at,
                        now + *midi_note_frames,
                        key,
                        velocity,
                    );
                }
            },
        );
        for (key, due) in self.note_off_at.iter_mut().enumerate() {
            if *due <= now {
                self.synth.note_off(DRUM_CHANNEL, key as i32);
                *due = NO_NOTE_OFF;
            }
        }
        if let Some(at) = at {
            for (key, due) in self.midi_off_at.iter_mut().enumerate() {
                if *due <= now {
                    *due = NO_NOTE_OFF;
                    push_midi(&mut self.midi, at, [MIDI_NOTE_OFF, key as u8, 0], 3);
                }
            }
        }
        self.synth.render(&mut self.left, &mut self.right);
        if self.fade_left > 0 {
            self.fade_block();
        }
        self.volume_block();
        if self.midi_out {
            // The synthesizer keeps running so its state stays the same as with sound, but the
            // MIDI device plays the notes instead.
            self.left.fill(0.0);
            self.right.fill(0.0);
        }
        self.read = 0;
        self.clock += block;
    }

    /// Plays the queued live hits now, as [`Engine::hit`] would but scaled by their row's gain,
    /// and reports each one, played or not, to the heard hit queue. A report that does not fit
    /// is dropped.
    fn play_live_hits(&mut self) {
        while let Ok(hit) = self.live.pop() {
            let key = hit.key.min(127);
            if hit.sound {
                let velocity = self
                    .sequencer
                    .live_velocity(key, hit.velocity.clamp(1, 127));
                if velocity > 0 {
                    self.note_on(key, velocity);
                }
            }
            let frame = self.heard_frame(hit.at);
            let heard = HeardHit {
                key: hit.key,
                velocity: hit.velocity,
                position: self.sequencer.heard_position(frame),
            };
            // A full queue means nothing reads the reports; the hit has played anyway.
            let _ = self.heard.push(heard);
        }
    }

    /// The frame the listener heard at `at`. The frame `call_start` is heard one output latency
    /// after `callback_now`, the start of the current `render` call, and frames are heard at the
    /// sample rate, so the frame heard at `at` is `call_start - latency` plus the frames between
    /// `callback_now` and `at`. It may lie before frame 0.
    fn heard_frame(&mut self, at: Instant) -> f64 {
        let now = *self.callback_now.get_or_insert_with(Instant::now);
        let seconds = match now.checked_duration_since(at) {
            Some(before) => -before.as_secs_f64(),
            None => (at - now).as_secs_f64(),
        };
        self.call_start as f64 - self.latency_frames as f64 + seconds * self.sample_rate
    }

    /// The moment the first frame of the current block will be heard: the start of the current
    /// `render` call, plus the output latency, plus the frames written before the block.
    fn block_time(&mut self) -> Instant {
        let callback_now = *self.callback_now.get_or_insert_with(Instant::now);
        let frames = self.latency_frames + self.block_offset as u64;
        callback_now + Duration::from_secs_f64(frames as f64 / self.sample_rate)
    }

    /// Queues a MIDI message of `len` bytes, due when the current block is heard, if MIDI out
    /// is on.
    fn send_midi(&mut self, bytes: [u8; 3], len: u8) {
        if self.midi_out {
            let at = self.block_time();
            push_midi(&mut self.midi, at, bytes, len);
        }
    }

    /// Turns MIDI out on or off. Turning it on queues a program change for the current kit and
    /// the channel volume first. Turning it off queues a note-off for every key that still has one pending, so no
    /// note hangs on the MIDI device, and cuts the notes the silent synthesizer still plays.
    fn set_midi_out(&mut self, on: bool) {
        if on {
            self.midi_out = true;
            self.send_midi([MIDI_PROGRAM_CHANGE, self.program, 0], 2);
            self.send_midi(
                [MIDI_CONTROL_CHANGE, MIDI_CHANNEL_VOLUME, self.midi_volume],
                3,
            );
        } else if self.midi_out {
            for key in 0..=127_u8 {
                if self.midi_off_at[usize::from(key)] != NO_NOTE_OFF {
                    self.midi_off_at[usize::from(key)] = NO_NOTE_OFF;
                    self.send_midi([MIDI_NOTE_OFF, key, 0], 3);
                }
            }
            // The synthesizer kept playing those notes in silence. Cut them, so their tails do
            // not appear out of nowhere now that its output is heard again.
            self.synth.note_off_all(true);
            self.midi_out = false;
        }
    }

    /// Applies one command at frame `self.clock`, the start of the next block.
    fn apply(&mut self, command: Command) {
        if command.is_transport() {
            self.transport = self.transport.wrapping_add(1);
        }
        match command {
            Command::NoteOn { key, velocity } => self.note_on(key, velocity),
            Command::ProgramChange { program } => {
                self.synth
                    .process_midi_message(DRUM_CHANNEL, 0xC0, i32::from(program), 0);
                self.program = program.min(127);
                self.send_midi([MIDI_PROGRAM_CHANGE, self.program, 0], 2);
            }
            Command::SetMidiOut { on } => self.set_midi_out(on),
            Command::Silence => self.silence(),
            Command::SetMidiVolume { volume } => {
                // The synthesizer does not hear it: the volume is the MIDI device's alone.
                self.midi_volume = volume.min(127);
                self.send_midi(
                    [MIDI_CONTROL_CHANGE, MIDI_CHANNEL_VOLUME, self.midi_volume],
                    3,
                );
            }
            Command::SetVolume { gain } => self.volume = clamp_gain(gain),
            other => self.sequencer.apply(other, self.clock),
        }
    }

    /// Scales the block just rendered by the output volume, ramping linearly from the gain the
    /// last block ended on to the one set now. Does nothing at full volume.
    fn volume_block(&mut self) {
        let from = self.volume_reached;
        let to = self.volume;
        if from == 1.0 && to == 1.0 {
            return;
        }
        let frames = self.left.len() as f32;
        for (i, (l, r)) in self.left.iter_mut().zip(self.right.iter_mut()).enumerate() {
            let gain = from + (to - from) * ((i + 1) as f32 / frames);
            *l *= gain;
            *r *= gain;
        }
        self.volume_reached = to;
    }

    /// Starts a fade of every sound still ringing, unless one already runs, and sends MIDI out's
    /// pending note-offs now.
    fn silence(&mut self) {
        if self.fade_left == 0 {
            self.fade_left = self.fade_frames;
        }
        if self.midi_out {
            for key in 0..=127_u8 {
                if self.midi_off_at[usize::from(key)] != NO_NOTE_OFF {
                    self.midi_off_at[usize::from(key)] = NO_NOTE_OFF;
                    self.send_midi([MIDI_NOTE_OFF, key, 0], 3);
                }
            }
        }
    }

    /// Scales the block just rendered by the fade, which falls linearly from where it is to 0,
    /// and cuts every sound when the fade ends.
    fn fade_block(&mut self) {
        let total = self.fade_frames as f32;
        let mut left = self.fade_left;
        for (l, r) in self.left.iter_mut().zip(self.right.iter_mut()) {
            left = left.saturating_sub(1);
            let gain = left as f32 / total;
            *l *= gain;
            *r *= gain;
        }
        self.fade_left = left;
        if left == 0 {
            self.cut();
        }
    }

    /// Ends every sound at once, reverb and chorus tails included, and forgets their note-offs.
    /// Keeps the kit.
    fn cut(&mut self) {
        self.synth.reset();
        self.synth
            .process_midi_message(DRUM_CHANNEL, 0xC0, i32::from(self.program), 0);
        self.note_off_at.fill(NO_NOTE_OFF);
        self.fade_left = 0;
    }

    /// Starts a drum hit now and moves its key's note-off deadlines. A fade still running is
    /// ended first, so the hit is heard whole.
    fn note_on(&mut self, key: u8, velocity: u8) {
        if self.fade_left > 0 {
            self.cut();
        }
        self.synth
            .note_on(DRUM_CHANNEL, i32::from(key), i32::from(velocity));
        self.note_off_at[usize::from(key.min(127))] = self.clock + self.ring_frames;
        if self.midi_out {
            let at = self.block_time();
            let off = self.clock + self.midi_note_frames;
            midi_note_on(
                &mut self.midi,
                &mut self.midi_off_at,
                at,
                off,
                key,
                velocity,
            );
        }
    }
}

/// Queues a MIDI note-on due at `at`, and sets the key's MIDI note-off to frame `off`. If the
/// key's previous note has not ended yet, its note-off goes first, so the device never gets two
/// note-ons for a key without a note-off between them.
fn midi_note_on(
    midi: &mut rtrb::Producer<MidiEvent>,
    midi_off_at: &mut [u64; 128],
    at: Instant,
    off: u64,
    key: u8,
    velocity: u8,
) {
    let due = &mut midi_off_at[usize::from(key.min(127))];
    if *due != NO_NOTE_OFF {
        push_midi(midi, at, [MIDI_NOTE_OFF, key, 0], 3);
    }
    *due = off;
    push_midi(midi, at, [MIDI_NOTE_ON, key, velocity], 3);
}

/// Queues the first `len` bytes of a MIDI message, due at `at`. The data bytes are kept in range
/// (0..=127). If the queue is full, the message is dropped: the audio thread must not wait, and a
/// full queue means nothing is reading it.
fn push_midi(midi: &mut rtrb::Producer<MidiEvent>, at: Instant, bytes: [u8; 3], len: u8) {
    let [status, data1, data2] = bytes;
    let event = MidiEvent {
        at,
        bytes: [status, data1 & 0x7F, data2 & 0x7F],
        len,
    };
    let _ = midi.push(event);
}

/// A running audio output playing General MIDI drums on channel 10.
pub struct Engine {
    commands: Commands,
    status: Arc<Status>,
    /// How many play and stop commands have been sent, wrapping.
    transport_sent: u32,
    /// Whether the last play or stop command sent starts the project's play.
    expect_playing: bool,
    /// Whether the last play or stop command sent starts the preview.
    expect_previewing: bool,
    /// The number of song entries last sent.
    song_len: usize,
    sample_rate: u32,
    channels: u16,
    buffer_frames: Option<u32>,
    /// The kits of the bank the engine started with.
    kits: Vec<Kit>,
    /// The receiving end of the MIDI queue, until [`Engine::take_midi_events`] hands it out.
    midi_events: Option<MidiEvents>,
    /// The sending end of the live hit queue, until [`Engine::take_live_hits`] hands it out.
    live_hits: Option<LiveHits>,
    /// The receiving end of the heard hit queue.
    heard_hits: Option<rtrb::Consumer<HeardHit>>,
    // Declared last so it is dropped last; dropping it stops the callback.
    _stream: OutputStream,
}

/// The stream that keeps the audio callback running: a cpal stream, or on Android an AAudio
/// stream opened by [`aaudio`].
#[cfg(not(target_os = "android"))]
type OutputStream = cpal::Stream;
#[cfg(target_os = "android")]
type OutputStream = aaudio::Stream;

/// A running output stream, the queue that feeds its renderer and what the stream reports.
struct OpenOutput {
    stream: OutputStream,
    commands: Commands,
    status: Arc<Status>,
    sample_rate: u32,
    channels: u16,
    buffer_frames: Option<u32>,
    midi_events: Option<MidiEvents>,
    live_hits: Option<LiveHits>,
    heard_hits: Option<rtrb::Consumer<HeardHit>>,
}

impl Engine {
    /// Opens the default output device and starts rendering silence.
    ///
    /// On Android the output is an AAudio stream that asks for low-latency mode first and falls
    /// back to the default mode; see [`aaudio`]. Elsewhere it is cpal's default output device.
    pub fn start(soundfont: Arc<SoundFont>) -> Result<Self, AudioError> {
        let output = open_output(&soundfont)?;
        Ok(Engine {
            commands: output.commands,
            status: output.status,
            transport_sent: 0,
            expect_playing: false,
            expect_previewing: false,
            song_len: 0,
            sample_rate: output.sample_rate,
            channels: output.channels,
            buffer_frames: output.buffer_frames,
            kits: drum_kits(&soundfont),
            midi_events: output.midi_events,
            live_hits: output.live_hits,
            heard_hits: output.heard_hits,
            _stream: output.stream,
        })
    }

    /// Plays one drum hit. `key` is the General MIDI drum key (36 is the kick), `velocity` 1..=127.
    /// Never blocks; a hit is dropped if the queue is full.
    pub fn hit(&mut self, key: u8, velocity: u8) {
        self.commands.hit(key, velocity);
    }

    /// Selects the drum kit: a program number in the percussion bank, as listed by
    /// [`Engine::kits`]. It applies to sequenced notes and to [`Engine::hit`] from the next note
    /// on. A program the bank lacks plays the standard kit, program 0; see the crate
    /// documentation.
    pub fn set_kit(&mut self, program: u8) {
        self.commands.set_kit(program);
    }

    /// Hands out the receiving end of the MIDI queue. The first call returns it; later calls
    /// return `None`.
    pub fn take_midi_events(&mut self) -> Option<MidiEvents> {
        self.midi_events.take()
    }

    /// Hands out the sending end of the live hit queue, for the MIDI input's thread. The first
    /// call returns it; later calls return `None`.
    ///
    /// The audio thread plays each hit at the start of its next block, as [`Engine::hit`] does,
    /// but at the velocity scaled by the gain of the first row with the hit's key, so a muted
    /// row stays silent. With no row for the key, the hit plays at its own velocity. A hit with
    /// `sound` false is not played. Every hit, played or not, is reported to
    /// [`Engine::take_heard_hits`]. A hit pushed into a full queue is lost.
    pub fn take_live_hits(&mut self) -> Option<LiveHits> {
        self.live_hits.take()
    }

    /// Takes the live hits the audio thread has handled since the last call, oldest first, each
    /// with the position the player was hearing when they hit. The interface calls it once per
    /// frame. The queue holds [`LIVE_QUEUE_CAPACITY`] reports; more are dropped until it is read.
    ///
    /// The position comes from the hit's time. The audio thread knows when the first frame of
    /// its buffer will be heard: one output latency after its callback starts. From that it
    /// works out which frame the player heard at the hit's time, and where the sequencer was at
    /// that frame. The step is in straight time, without swing.
    pub fn take_heard_hits(&mut self) -> Vec<HeardHit> {
        let Some(heard) = self.heard_hits.as_mut() else {
            return Vec::new();
        };
        std::iter::from_fn(|| heard.pop().ok()).collect()
    }

    /// Turns MIDI out on or off. While on, the audio thread queues a MIDI message for every note
    /// it plays and the internal synthesizer is silent. Turning it on also queues a program
    /// change for the current kit and the channel volume.
    pub fn set_midi_out(&mut self, on: bool) {
        self.commands.send(Command::SetMidiOut { on });
    }

    /// Sets the channel volume MIDI out sends, 0..=127 (controller 7 on the drum channel). It is
    /// sent now if MIDI out is on, and again each time MIDI out is turned on, because the device
    /// keeps the last value it got, from any program. It does not change the internal
    /// synthesizer. A new engine starts at [`DEFAULT_MIDI_VOLUME`].
    pub fn set_midi_volume(&mut self, volume: u8) {
        self.commands.send(Command::SetMidiVolume {
            volume: volume.min(127),
        });
    }

    /// Sets the gain of everything the engine plays, 0.0..=1.0 (see "Volume" in the crate
    /// documentation). It scales the sound, not the velocities, and is reached over one block.
    /// A new engine starts at 1.0.
    pub fn set_volume(&mut self, gain: f32) {
        self.commands.send(Command::SetVolume {
            gain: clamp_gain(gain),
        });
    }

    /// The kits of the bank the engine started with, sorted by program.
    pub fn kits(&self) -> &[Kit] {
        &self.kits
    }

    /// Sets the gain of drum `row`, in every pattern, clamped to 0.0..=1.0. Every row starts at
    /// 1.0. A sequenced note plays at `round(velocity * gain)`, and not at all if that is 0, so
    /// gain 0 mutes the row. Previews from [`Engine::hit`] are not affected. The change counts
    /// from the next step that fires. `row` must be below [`MAX_ROWS`]; larger values are
    /// ignored, and so is a gain that is not a number.
    pub fn set_row_gain(&mut self, row: usize, gain: f32) {
        self.commands.send_some(Command::row_gain(row, gain));
    }

    /// Turns off every step of every row of one pattern, in one command. The length is kept.
    /// `pattern` must be below [`MAX_PATTERNS`]; larger values are ignored.
    pub fn clear_pattern(&mut self, pattern: usize) {
        self.commands.send_some(Command::clear_pattern(pattern));
    }

    /// Sets which drum `row` plays, in every pattern. Rows start silent until given a key.
    /// `row` must be below [`MAX_ROWS`]; larger values are ignored.
    pub fn set_row_key(&mut self, row: usize, key: u8) {
        self.commands.send_some(Command::row_key(row, key));
    }

    /// Turns one step of one pattern on with `velocity` (1..=127), or off with 0. `pattern` must
    /// be below [`MAX_PATTERNS`], `row` below [`MAX_ROWS`] and `step` below [`MAX_STEPS`]; other
    /// values are ignored.
    pub fn set_step(&mut self, pattern: usize, row: usize, step: usize, velocity: u8) {
        self.commands
            .send_some(Command::step(pattern, row, step, velocity));
    }

    /// Sets how many steps a pattern plays, clamped to 1..=[`MAX_STEPS`]. Steps beyond the
    /// length are kept but not played. Every pattern starts at 16 steps. `pattern` must be below
    /// [`MAX_PATTERNS`]; larger values are ignored.
    pub fn set_length(&mut self, pattern: usize, steps: usize) {
        self.commands.send_some(Command::length(pattern, steps));
    }

    /// Sets the tempo in beats per minute, clamped to [`MIN_BPM`]..=[`MAX_BPM`]. A beat is four
    /// steps. Takes effect at once, also while playing. A value that is not a number is ignored.
    pub fn set_tempo(&mut self, bpm: f32) {
        self.commands.send_some(Command::tempo(bpm));
    }

    /// Sets the swing, clamped to 0.0..=[`MAX_SWING`]; a new engine has 0, straight timing.
    /// Every odd step of a pattern (1, 3, 5 and so on) sounds `amount` of a step later than its
    /// straight time; even steps keep theirs. 0.33 gives a triplet shuffle. Takes effect from
    /// the next step, also while playing. A value that is not a number is ignored.
    pub fn set_swing(&mut self, amount: f32) {
        self.commands.send_some(Command::swing(amount));
    }

    /// Replaces the song order: the pattern slot of each entry, in order. Entries beyond
    /// [`MAX_SONG`] are dropped, and a slot at or above [`MAX_PATTERNS`] plays the last slot.
    /// While the song plays, the current entry plays to its end before the new order counts.
    pub fn set_song(&mut self, order: &[usize]) {
        self.song_len = self.commands.set_song(order);
    }

    /// Sets whether the song starts again at entry 0 after its last entry. A new engine's song
    /// does not loop.
    pub fn set_song_loop(&mut self, looped: bool) {
        self.commands.send(Command::SetSongLoop { looped });
    }

    /// Loops `hits` over `steps` steps on the preview track, from step 0, starting now. The
    /// preview is separate from the project: its patterns, rows and song are not changed. It
    /// plays at the engine's tempo and swing, with the current kit. See the crate documentation.
    ///
    /// `steps` is clamped to 1..=[`MAX_STEPS`] (16 bars of 16 steps). A hit's `step` counts from
    /// the start of the preview. Hits at or after `steps`, with a key above 127 or with velocity
    /// 0 are ignored. A velocity above 127 plays at 127. The keys play directly, without row
    /// gain. Of two hits on the same step and key, the louder one plays.
    ///
    /// Stops the song or pattern that plays. Calling it again replaces the preview and starts it
    /// again from step 0. Sounds already started ring out.
    pub fn preview(&mut self, hits: Vec<PreviewHit>, steps: usize) {
        for command in Command::preview(&hits, steps) {
            if command.is_transport() {
                self.send_transport(command);
            } else {
                self.commands.send(command);
            }
        }
        self.expect_playing = false;
        self.expect_previewing = true;
    }

    /// Stops the preview, if one plays. Sounds already started ring out. Leaves the project's
    /// play alone.
    pub fn stop_preview(&mut self) {
        self.send_transport(Command::StopPreview);
        self.expect_previewing = false;
    }

    /// Whether the preview plays. True from the moment [`Engine::preview`] is called, and false
    /// from the moment [`Engine::stop_preview`], [`Engine::stop`] or a project play call is made.
    pub fn previewing(&self) -> bool {
        if self.status.transport.load(Ordering::Acquire) == self.transport_sent {
            self.status.previewing.load(Ordering::Relaxed)
        } else {
            self.expect_previewing
        }
    }

    /// Loops one pattern from its step 0, starting now. Leaves song mode if the song was playing,
    /// and stops the preview. `pattern` must be below [`MAX_PATTERNS`]; larger values are
    /// ignored.
    pub fn play_pattern(&mut self, pattern: usize) {
        self.play_pattern_from(pattern, 0);
    }

    /// Like [`Engine::play_pattern`], but starts at step `from`, such as the first step of a
    /// bar. A step past the pattern's end starts at step 0.
    pub fn play_pattern_from(&mut self, pattern: usize, from: usize) {
        if pattern < MAX_PATTERNS {
            self.send_transport(Command::PlayPattern {
                pattern: pattern as u8,
                from: from.min(usize::from(u16::MAX)) as u16,
            });
            self.expect_playing = true;
            self.expect_previewing = false;
        }
    }

    /// While a pattern loops, switches to `pattern` at the next bar boundary: play goes on in
    /// `pattern` at the bar after the current one, wrapped to `pattern`'s bar count. Cueing the
    /// pattern that is playing cancels an earlier cue. Does nothing while stopped or while the
    /// song plays.
    pub fn cue_pattern(&mut self, pattern: usize) {
        if pattern < MAX_PATTERNS {
            self.commands.send(Command::CuePattern {
                pattern: pattern as u8,
            });
        }
    }

    /// While a pattern loops, plays only bar `bar` of it, over and over; `None` plays the whole
    /// pattern again. Bars count from 0 and are 16 steps long; the last bar of a pattern may be
    /// shorter.
    ///
    /// If play is outside that bar, it moves into the bar at the next step, at the same step
    /// within a bar, so the change is heard at once and stays in time. A bar at or past the
    /// playing pattern's bar count acts as `None`. A cued pattern ([`Engine::cue_pattern`])
    /// starts at the first step of its own copy of the bar. The setting holds across
    /// [`Engine::play_pattern_from`], cues and [`Engine::stop`]: a play call that starts outside
    /// the bar starts inside it instead, at the same step within a bar. [`Engine::play_song`]
    /// clears it. Song mode and the preview ignore it.
    pub fn set_loop_bar(&mut self, bar: Option<usize>) {
        self.commands.send(Command::SetLoopBar {
            bar: bar.map(|bar| bar.min(usize::from(u8::MAX)) as u8),
        });
    }

    /// Plays the song from entry `from_index`, step 0, starting now, and stops the preview. Clears
    /// the loop bar ([`Engine::set_loop_bar`]). Does nothing if the song has no entry at
    /// `from_index`, which includes an empty song.
    pub fn play_song(&mut self, from_index: usize) {
        if from_index < self.song_len {
            self.send_transport(Command::PlaySong {
                from: from_index as u8,
            });
            self.expect_playing = true;
            self.expect_previewing = false;
        }
    }

    /// Stops play, also of the preview, and silences every sound still ringing, as
    /// [`Engine::silence`] does.
    pub fn stop(&mut self) {
        self.send_transport(Command::Stop);
        self.silence();
        self.expect_playing = false;
        self.expect_previewing = false;
    }

    /// Fades out every sound still ringing over [`SILENCE_SECONDS`], also hits played with
    /// [`Engine::hit`], and sends MIDI out's pending note-offs now. Leaves play alone: hits
    /// played after it sound as usual.
    pub fn silence(&mut self) {
        self.commands.send(Command::Silence);
    }

    fn send_transport(&mut self, command: Command) {
        self.commands.send(command);
        self.transport_sent = self.transport_sent.wrapping_add(1);
    }

    /// The position the listener is hearing now, or `None` when stopped or while the preview
    /// plays. Allowing for the output latency, so the interface can highlight the step in time
    /// with the sound.
    ///
    /// The audio callback updates it once per device buffer. Reading it is one atomic load.
    pub fn playhead(&self) -> Option<Playhead> {
        Playhead::unpack(self.status.playhead.load(Ordering::Relaxed))
    }

    /// Whether the sequencer plays a project pattern or the song. True from the moment a play call
    /// is made, and false from the moment [`Engine::stop`] or [`Engine::preview`] is called or a
    /// song that does not loop has reached the end of its last step. That end is rendered up to
    /// one output latency before it is heard. The preview does not count; see
    /// [`Engine::previewing`].
    pub fn is_playing(&self) -> bool {
        if self.status.transport.load(Ordering::Acquire) == self.transport_sent {
            self.status.playing.load(Ordering::Relaxed)
        } else {
            self.expect_playing
        }
    }

    /// Sends commands that did not fit in the queue earlier. Returns whether all have been sent.
    ///
    /// Every other engine call does this first, so it is only needed when the interface may make
    /// no other call for a while after a large burst of changes. Calling it once per interface
    /// frame is cheap.
    pub fn flush(&mut self) -> bool {
        self.commands.flush()
    }

    /// The output latency the playhead allows for, in frames: the time from the audio callback
    /// to the sound leaving the device, as the stream timestamps report it (cpal's, or AAudio's
    /// on Android), or the length of the callback's buffer when they do not. Zero until the
    /// first callback.
    pub fn output_latency_frames(&self) -> u32 {
        self.status.latency_frames.load(Ordering::Relaxed)
    }

    /// The output sample rate, which is also the synthesizer's.
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// The number of output channels of the device.
    pub fn channels(&self) -> u16 {
        self.channels
    }

    /// The device buffer size in frames, if the backend reports it.
    pub fn buffer_frames(&self) -> Option<u32> {
        self.buffer_frames
    }
}

// The targets this proof of concept ships on must be able to move the engine to another thread.
#[cfg(any(windows, target_os = "android"))]
const _: fn() = || {
    fn assert_send<T: Send>() {}
    assert_send::<Engine>();
};

/// Converts an output latency in seconds to frames. Falls back to `fallback_frames` if
/// `seconds` is zero or less, or more than `MAX_PLAUSIBLE_LATENCY_SECONDS`.
fn latency_frames(seconds: f64, sample_rate: f64, fallback_frames: usize) -> u32 {
    if seconds > 0.0 && seconds <= MAX_PLAUSIBLE_LATENCY_SECONDS {
        (seconds * sample_rate).round() as u32
    } else {
        u32::try_from(fallback_frames).unwrap_or(u32::MAX)
    }
}

/// The output latency of one callback in frames: from the callback to the predicted playback of
/// its first frame. Falls back to `buffer_frames`, the length of the callback's buffer, if the
/// timestamps give zero or more than `MAX_PLAUSIBLE_LATENCY_SECONDS`.
#[cfg(not(target_os = "android"))]
fn callback_latency(
    info: &cpal::OutputCallbackInfo,
    sample_rate: f64,
    buffer_frames: usize,
) -> u32 {
    let timestamp = info.timestamp();
    let seconds = timestamp
        .playback
        .duration_since(timestamp.callback)
        .as_secs_f64();
    latency_frames(seconds, sample_rate, buffer_frames)
}

/// Opens cpal's default output device and starts a stream that the renderer fills.
#[cfg(not(target_os = "android"))]
fn open_output(soundfont: &Arc<SoundFont>) -> Result<OpenOutput, AudioError> {
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or_else(|| AudioError::Device("no default output device".into()))?;
    let supported = device
        .default_output_config()
        .map_err(|e| AudioError::Device(format!("default output config: {e}")))?;
    let format = supported.sample_format();
    let config: cpal::StreamConfig = supported.into();
    let (commands, mut renderer) = Renderer::new(soundfont, config.sample_rate)?;
    let status = Arc::clone(&renderer.status);
    let midi_events = renderer.take_midi_events();
    let live_hits = renderer.take_live_hits();
    let heard_hits = renderer.take_heard_hits();

    let stream = match format {
        SampleFormat::F32 => build::<f32>(&device, config, renderer),
        SampleFormat::I16 => build::<i16>(&device, config, renderer),
        SampleFormat::I32 => build::<i32>(&device, config, renderer),
        SampleFormat::U16 => build::<u16>(&device, config, renderer),
        SampleFormat::F64 => build::<f64>(&device, config, renderer),
        other => Err(AudioError::Device(format!(
            "unsupported sample format {other}"
        ))),
    }?;
    stream
        .play()
        .map_err(|e| AudioError::Device(format!("start the stream: {e}")))?;

    let buffer_frames = stream.buffer_size().ok();
    log::info!(
        "audio output: {} Hz, {} channels, {format}, device buffer {:?} frames",
        config.sample_rate,
        config.channels,
        buffer_frames
    );
    Ok(OpenOutput {
        stream,
        commands,
        status,
        sample_rate: config.sample_rate,
        channels: config.channels,
        buffer_frames,
        midi_events,
        live_hits,
        heard_hits,
    })
}

#[cfg(target_os = "android")]
use aaudio::open_output;

/// Builds an output stream of sample type `T` that the renderer fills.
#[cfg(not(target_os = "android"))]
fn build<T>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    mut renderer: Renderer,
) -> Result<cpal::Stream, AudioError>
where
    T: SizedSample + FromSample<f32>,
{
    let channels = usize::from(config.channels);
    let sample_rate = f64::from(config.sample_rate);
    device
        .build_output_stream(
            config,
            move |data: &mut [T], info: &cpal::OutputCallbackInfo| {
                let frames = data.len() / channels.max(1);
                renderer.set_latency(callback_latency(info, sample_rate, frames));
                renderer.render(data, channels);
            },
            |e| log::warn!("audio stream error: {e}"),
            None,
        )
        .map_err(|e| AudioError::Device(format!("build the stream: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::OnceLock;

    const RATE: u32 = 48_000;
    const BLOCK: u64 = 64;

    /// Loads the bank named by `DM_TEST_SF2` once for all tests, or returns `None` if it is unset.
    pub(crate) fn soundfont() -> Option<Arc<SoundFont>> {
        static FONT: OnceLock<Option<Arc<SoundFont>>> = OnceLock::new();
        FONT.get_or_init(|| {
            let path = std::env::var_os("DM_TEST_SF2")?;
            Some(load_soundfont(Path::new(&path)).expect("DM_TEST_SF2 names a loadable bank"))
        })
        .clone()
    }

    macro_rules! soundfont_or_skip {
        () => {
            match soundfont() {
                Some(font) => font,
                None => {
                    println!("DM_TEST_SF2 is not set; skipping");
                    return;
                }
            }
        };
    }

    pub(crate) fn energy(samples: &[f32]) -> f32 {
        (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt()
    }

    /// A position in pattern mode.
    fn pat(pattern: usize, step: usize) -> Playhead {
        Playhead {
            pattern,
            step,
            song_index: None,
        }
    }

    /// A position in song mode.
    fn entry(index: usize, pattern: usize, step: usize) -> Playhead {
        Playhead {
            pattern,
            step,
            song_index: Some(index),
        }
    }

    /// Runs `sequencer` over blocks from frame `from` up to frame `to` and returns the
    /// `(block start, position)` of every step that fired.
    fn run(sequencer: &mut Sequencer, from: u64, to: u64) -> Vec<(u64, Playhead)> {
        let mut fired = Vec::new();
        let mut start = from;
        while start < to {
            sequencer.advance(start, BLOCK, |p| fired.push((start, p)), |_, _| {});
            start += BLOCK;
        }
        fired
    }

    /// The steps of `fired`, without frames or patterns.
    fn steps(fired: &[(u64, Playhead)]) -> Vec<usize> {
        fired.iter().map(|(_, p)| p.step).collect()
    }

    /// The start of the block that contains `frame`.
    fn block_of(frame: f64) -> u64 {
        (frame / BLOCK as f64).floor() as u64 * BLOCK
    }

    /// A sequencer with pattern `pattern` playing from frame 0.
    fn playing(pattern: u8) -> Sequencer {
        let mut sequencer = Sequencer::new(RATE);
        sequencer.apply(Command::PlayPattern { pattern, from: 0 }, 0);
        sequencer
    }

    /// Sets the song order through the command queue, like the engine does.
    fn set_song(sequencer: &mut Sequencer, order: &[usize], now: u64) {
        let (mut commands, mut consumer) = command_queue();
        commands.set_song(order);
        drain(&mut consumer, sequencer, now);
    }

    /// Pops every queued command into `sequencer` at frame `now`, like the renderer does.
    fn drain(consumer: &mut rtrb::Consumer<Command>, sequencer: &mut Sequencer, now: u64) {
        while let Ok(command) = consumer.pop() {
            sequencer.apply(command, now);
        }
    }

    #[test]
    fn steps_fire_every_6000_frames_at_120_bpm() {
        let mut sequencer = playing(0);
        sequencer.apply(Command::SetTempo { bpm: 120.0 }, 0);
        assert_eq!(sequencer.clock.frames_per_step, 6000.0);
        let fired = run(&mut sequencer, 0, 6 * 6000 - BLOCK);
        let expected: Vec<_> = (0..6)
            .map(|k| (block_of(k as f64 * 6000.0), pat(0, k)))
            .collect();
        assert_eq!(fired, expected);
    }

    #[test]
    fn a_pattern_can_start_at_a_later_step_and_wraps_to_its_start() {
        let mut sequencer = Sequencer::new(RATE);
        sequencer.apply(
            Command::SetLength {
                pattern: 0,
                steps: 32,
            },
            0,
        );
        sequencer.apply(
            Command::PlayPattern {
                pattern: 0,
                from: 16,
            },
            0,
        );
        let fired = run(&mut sequencer, 0, 18 * 6000 - BLOCK);
        let expected: Vec<usize> = (16..32).chain(0..2).collect();
        assert_eq!(steps(&fired), expected);
        // A step past the end starts at the beginning.
        let mut sequencer = Sequencer::new(RATE);
        sequencer.apply(
            Command::PlayPattern {
                pattern: 0,
                from: 999,
            },
            0,
        );
        assert_eq!(steps(&run(&mut sequencer, 0, 6000 - BLOCK)), [0]);
    }

    #[test]
    fn the_pattern_wraps_at_its_length() {
        let mut sequencer = playing(0);
        sequencer.apply(
            Command::SetLength {
                pattern: 0,
                steps: 3,
            },
            0,
        );
        let fired = run(&mut sequencer, 0, 7 * 6000 - BLOCK);
        assert_eq!(steps(&fired), [0, 1, 2, 0, 1, 2, 0]);
    }

    #[test]
    fn each_slot_loops_its_own_length() {
        let mut sequencer = Sequencer::new(RATE);
        sequencer.apply(
            Command::SetLength {
                pattern: 1,
                steps: 5,
            },
            0,
        );
        sequencer.apply(
            Command::SetLength {
                pattern: 2,
                steps: 32,
            },
            0,
        );
        for (pattern, length) in [(0, 16), (1, 5), (2, 32)] {
            sequencer.apply(Command::PlayPattern { pattern, from: 0 }, 0);
            let fired = run(&mut sequencer, 0, 70 * 6000 - BLOCK);
            assert_eq!(fired.len(), 70);
            for (k, (_, position)) in fired.iter().enumerate() {
                assert_eq!(*position, pat(usize::from(pattern), k % length));
            }
            sequencer.apply(Command::Stop, 70 * 6000);
        }
    }

    #[test]
    fn a_shorter_length_wraps_the_position() {
        let mut sequencer = playing(0);
        let fired = run(&mut sequencer, 0, 10 * 6000 - BLOCK);
        assert_eq!(fired.last().unwrap().1.step, 9);
        sequencer.apply(
            Command::SetLength {
                pattern: 0,
                steps: 8,
            },
            10 * 6000 - BLOCK,
        );
        let fired = run(&mut sequencer, 10 * 6000 - BLOCK, 12 * 6000 - BLOCK);
        // Step 10 would have been next; it wraps to 10 % 8 = 2.
        assert_eq!(steps(&fired), [2, 3]);
    }

    #[test]
    fn a_tempo_change_mid_step_keeps_the_proportion() {
        let mut sequencer = playing(0);
        assert_eq!(run(&mut sequencer, 0, 3008), [(0, pat(0, 0))]);
        // At frame 3008, 2992 of 6000 frames of step 0 are left. Halving the tempo doubles them.
        sequencer.apply(Command::SetTempo { bpm: 60.0 }, 3008);
        let due = 3008.0 + 2992.0 * 2.0;
        let fired = run(&mut sequencer, 3008, 30_000);
        assert_eq!(
            fired,
            [
                (block_of(due), pat(0, 1)),
                (block_of(due + 12_000.0), pat(0, 2))
            ],
            "no step is skipped or repeated"
        );
    }

    #[test]
    fn play_restarts_at_step_zero_in_the_same_block() {
        let mut sequencer = playing(0);
        run(&mut sequencer, 0, 20_000);
        sequencer.apply(
            Command::PlayPattern {
                pattern: 0,
                from: 0,
            },
            20_032,
        );
        let fired = run(&mut sequencer, 20_032, 26_100);
        assert_eq!(
            fired,
            [(20_032, pat(0, 0)), (block_of(26_032.0), pat(0, 1))]
        );
    }

    #[test]
    fn stop_stops_scheduling() {
        let mut sequencer = playing(0);
        run(&mut sequencer, 0, 10_000);
        sequencer.apply(Command::Stop, 10_048);
        assert!(run(&mut sequencer, 10_048, 100_000).is_empty());
    }

    #[test]
    fn a_long_run_does_not_drift() {
        // 133 BPM at 44.1 kHz gives 4973.68... frames per step, which no integer can hold.
        let rate = 44_100;
        let mut sequencer = Sequencer::new(rate);
        sequencer.apply(Command::SetTempo { bpm: 133.0 }, 0);
        sequencer.apply(
            Command::SetLength {
                pattern: 0,
                steps: MAX_STEPS as u16,
            },
            0,
        );
        sequencer.apply(
            Command::PlayPattern {
                pattern: 0,
                from: 0,
            },
            0,
        );
        let per_step = f64::from(rate) * 60.0 / (133.0 * 4.0);
        let fired = run(&mut sequencer, 0, (10_000.0 * per_step) as u64 - BLOCK);
        assert_eq!(fired.len(), 10_000);
        for (k, &(start, position)) in fired.iter().enumerate() {
            assert_eq!(position.step, k % MAX_STEPS);
            assert_eq!(start, block_of(k as f64 * per_step), "step {k}");
        }
    }

    #[test]
    fn swing_delays_odd_steps_only() {
        let mut sequencer = playing(0);
        sequencer.apply(Command::swing(0.25).unwrap(), 0);
        let fired = run(&mut sequencer, 0, 6 * 6000 - BLOCK);
        let expected: Vec<_> = (0..6)
            .map(|k| {
                let late = if k % 2 == 1 { 1500.0 } else { 0.0 };
                (block_of(k as f64 * 6000.0 + late), pat(0, k))
            })
            .collect();
        assert_eq!(fired, expected);
        assert_eq!(fired[1].0, block_of(7500.0));
        assert_eq!(fired[2].0, block_of(12_000.0));
    }

    #[test]
    fn swing_counts_from_the_start_of_each_pattern() {
        // A three-step pattern plays 0 1 2 0 1 2: step 2 and the next step 0 are both straight.
        let mut sequencer = playing(0);
        sequencer.apply(
            Command::SetLength {
                pattern: 0,
                steps: 3,
            },
            0,
        );
        sequencer.apply(Command::swing(0.5).unwrap(), 0);
        let fired = run(&mut sequencer, 0, 6 * 6000 - BLOCK);
        let starts: Vec<_> = fired.iter().map(|&(start, _)| start).collect();
        let expected: Vec<_> = [0.0, 9000.0, 12_000.0, 18_000.0, 27_000.0, 30_000.0]
            .iter()
            .map(|&f| block_of(f))
            .collect();
        assert_eq!(steps(&fired), [0, 1, 2, 0, 1, 2]);
        assert_eq!(starts, expected);
    }

    #[test]
    fn a_long_swung_run_does_not_drift() {
        let rate = 44_100;
        let mut sequencer = Sequencer::new(rate);
        sequencer.apply(Command::SetTempo { bpm: 133.0 }, 0);
        sequencer.apply(Command::swing(0.33).unwrap(), 0);
        sequencer.apply(
            Command::PlayPattern {
                pattern: 0,
                from: 0,
            },
            0,
        );
        let per_step = f64::from(rate) * 60.0 / (133.0 * 4.0);
        let swing = f64::from(0.33_f32) * per_step;
        let fired = run(&mut sequencer, 0, (10_000.0 * per_step) as u64 - BLOCK);
        assert_eq!(fired.len(), 10_000);
        for (k, &(start, position)) in fired.iter().enumerate() {
            assert_eq!(position.step, k % 16);
            let late = if k % 2 == 1 { swing } else { 0.0 };
            assert_eq!(start, block_of(k as f64 * per_step + late), "step {k}");
        }
    }

    #[test]
    fn a_swing_change_between_steps_neither_skips_nor_repeats() {
        // Every run starts and ends on a whole block.
        let mut sequencer = playing(0);
        // Step 0 has fired. More swing delays step 1.
        assert_eq!(steps(&run(&mut sequencer, 0, 47 * BLOCK)), [0]);
        sequencer.apply(Command::swing(0.5).unwrap(), 47 * BLOCK);
        let fired = run(&mut sequencer, 47 * BLOCK, 186 * BLOCK);
        assert_eq!(fired, [(block_of(9000.0), pat(0, 1))]);
        // Step 1 has fired. Removing the swing leaves step 2 on the grid and moves step 3 back.
        sequencer.apply(Command::swing(0.0).unwrap(), 186 * BLOCK);
        let fired = run(&mut sequencer, 186 * BLOCK, 374 * BLOCK);
        assert_eq!(
            fired,
            [
                (block_of(12_000.0), pat(0, 2)),
                (block_of(18_000.0), pat(0, 3))
            ]
        );

        // Past the straight time of step 5 but before its swung time, the swing is removed:
        // step 5 fires in the next block, once, and step 6 stays on the grid.
        sequencer.apply(Command::swing(0.5).unwrap(), 374 * BLOCK);
        let now = 475 * BLOCK;
        assert!(now as f64 > 5.0 * 6000.0 && (now as f64) < 5.5 * 6000.0);
        let fired = run(&mut sequencer, 374 * BLOCK, now);
        assert_eq!(fired, [(block_of(24_000.0), pat(0, 4))]);
        sequencer.apply(Command::swing(0.0).unwrap(), now);
        let fired = run(&mut sequencer, now, 656 * BLOCK);
        assert_eq!(
            fired,
            [(now, pat(0, 5)), (block_of(36_000.0), pat(0, 6))],
            "no step is skipped or repeated"
        );
    }

    #[test]
    fn a_tempo_change_inside_the_swing_delay_fires_the_step_once() {
        let mut sequencer = playing(0);
        sequencer.apply(Command::swing(0.25).unwrap(), 0);
        // Step 1 is on the grid at 6000 and due at 7500; at 6400 it has not fired.
        let now = 100 * BLOCK;
        assert_eq!(steps(&run(&mut sequencer, 0, now)), [0]);
        // Half the tempo: the grid restarts at `now`, and step 1 keeps its swing delay of a
        // quarter of the new, longer step.
        sequencer.apply(Command::SetTempo { bpm: 60.0 }, now);
        let fired = run(&mut sequencer, now, now + 36_000 - BLOCK);
        let now = now as f64;
        assert_eq!(
            fired,
            [
                (block_of(now + 3000.0), pat(0, 1)),
                (block_of(now + 12_000.0), pat(0, 2)),
                (block_of(now + 27_000.0), pat(0, 3)),
            ]
        );
    }

    #[test]
    fn swing_is_clamped() {
        let amount = |command: Option<Command>| match command {
            Some(Command::SetSwing { amount }) => Some(amount),
            _ => None,
        };
        assert_eq!(amount(Command::swing(0.33)), Some(0.33));
        assert_eq!(amount(Command::swing(0.9)), Some(MAX_SWING));
        assert_eq!(amount(Command::swing(-1.0)), Some(0.0));
        assert_eq!(amount(Command::swing(f32::NAN)), None);
        let mut clock = Clock::new(RATE);
        clock.set_swing(f32::NAN);
        assert_eq!(clock.step_offset(1), 0.0);
        clock.set_swing(2.0);
        assert_eq!(clock.step_offset(1), 3000.0);
        assert_eq!(clock.step_offset(2), 0.0);
    }

    #[test]
    fn a_cue_switches_at_the_next_bar_boundary() {
        let mut sequencer = Sequencer::new(RATE);
        // Pattern 0 has two bars, pattern 1 three.
        sequencer.apply(
            Command::SetLength {
                pattern: 0,
                steps: 32,
            },
            0,
        );
        sequencer.apply(
            Command::SetLength {
                pattern: 1,
                steps: 48,
            },
            0,
        );
        sequencer.apply(
            Command::PlayPattern {
                pattern: 0,
                from: 0,
            },
            0,
        );
        // Into step 21 of pattern 0, which is in bar 1. Runs start and end on whole blocks.
        let now = 1971 * BLOCK;
        let fired = run(&mut sequencer, 0, now);
        assert_eq!(fired.last().unwrap().1, pat(0, 21));
        sequencer.apply(Command::CuePattern { pattern: 1 }, now);
        let fired = run(&mut sequencer, now, 40 * 6000 - BLOCK);
        let expected: Vec<_> = (22..40)
            .map(|k| {
                // Bar 1 of pattern 0 ends after step 31; play goes on in bar (1 + 1) % 3 = 2.
                let position = if k < 32 { pat(0, k) } else { pat(1, k) };
                (block_of(k as f64 * 6000.0), position)
            })
            .collect();
        assert_eq!(fired, expected);

        // Back to pattern 0 from bar 2 of pattern 1: bar (2 + 1) % 2 = 1, at step 48.
        sequencer.apply(Command::CuePattern { pattern: 0 }, 40 * 6000);
        let fired = run(&mut sequencer, 40 * 6000 - BLOCK, 50 * 6000 - BLOCK);
        let positions: Vec<_> = fired.iter().map(|&(_, p)| p).collect();
        let expected: Vec<_> = (40..50)
            .map(|k| if k < 48 { pat(1, k) } else { pat(0, k - 32) })
            .collect();
        assert_eq!(positions, expected);
    }

    #[test]
    fn a_cue_just_before_a_boundary_still_switches_there() {
        let mut sequencer = playing(0);
        // Step 15 has fired; step 16, the start of the next bar, has not.
        let now = 1408 * BLOCK;
        let fired = run(&mut sequencer, 0, now);
        assert_eq!(fired.last().unwrap().1, pat(0, 15));
        sequencer.apply(Command::CuePattern { pattern: 3 }, now);
        let fired = run(&mut sequencer, now, 17 * 6000 - BLOCK);
        // Both patterns have one bar, so bar (0 + 1) % 1 = 0.
        assert_eq!(fired, [(16 * 6000, pat(3, 0))]);
    }

    #[test]
    fn a_cue_of_the_playing_pattern_does_nothing_or_cancels() {
        let mut sequencer = playing(0);
        run(&mut sequencer, 0, 6000);
        sequencer.apply(Command::CuePattern { pattern: 0 }, 6000);
        let fired = run(&mut sequencer, 6000, 20 * 6000 - BLOCK);
        assert!(fired.iter().all(|(_, p)| p.pattern == 0));

        sequencer.apply(Command::CuePattern { pattern: 1 }, 20 * 6000);
        sequencer.apply(Command::CuePattern { pattern: 0 }, 20 * 6000);
        let fired = run(&mut sequencer, 20 * 6000 - BLOCK, 40 * 6000 - BLOCK);
        assert!(fired.iter().all(|(_, p)| p.pattern == 0));
    }

    #[test]
    fn a_cue_does_nothing_while_stopped_or_in_song_mode() {
        let mut sequencer = Sequencer::new(RATE);
        sequencer.apply(Command::CuePattern { pattern: 2 }, 0);
        assert_eq!(sequencer.cue, None);
        set_song(&mut sequencer, &[0], 0);
        sequencer.apply(Command::SetSongLoop { looped: true }, 0);
        sequencer.apply(Command::PlaySong { from: 0 }, 0);
        sequencer.apply(Command::CuePattern { pattern: 2 }, 0);
        let fired = run(&mut sequencer, 0, 40 * 6000 - BLOCK);
        assert_eq!(fired.len(), 40);
        assert!(fired.iter().all(|(_, p)| p.pattern == 0));
    }

    /// A sequencer whose pattern 0 is `steps` long, at 120 bpm (6000 frames a step).
    fn with_length(steps: u16) -> Sequencer {
        let mut sequencer = Sequencer::new(RATE);
        sequencer.apply(Command::SetLength { pattern: 0, steps }, 0);
        sequencer
    }

    #[test]
    fn the_loop_bar_wraps_within_the_bar_in_time() {
        let mut sequencer = with_length(48);
        sequencer.apply(Command::SetLoopBar { bar: Some(1) }, 0);
        sequencer.apply(
            Command::PlayPattern {
                pattern: 0,
                from: 16,
            },
            0,
        );
        let fired = run(&mut sequencer, 0, 40 * 6000 - BLOCK);
        let expected: Vec<_> = (0..40)
            .map(|k| (block_of(k as f64 * 6000.0), pat(0, 16 + k % 16)))
            .collect();
        assert_eq!(fired, expected);
    }

    #[test]
    fn the_loop_bar_ends_with_a_pattern_that_ends_inside_it() {
        let mut sequencer = with_length(20);
        sequencer.apply(Command::SetLoopBar { bar: Some(1) }, 0);
        sequencer.apply(
            Command::PlayPattern {
                pattern: 0,
                from: 16,
            },
            0,
        );
        let fired = run(&mut sequencer, 0, 10 * 6000 - BLOCK);
        assert_eq!(steps(&fired), [16, 17, 18, 19, 16, 17, 18, 19, 16, 17]);
    }

    #[test]
    fn a_loop_bar_set_elsewhere_moves_play_into_it_at_the_next_step() {
        let mut sequencer = with_length(32);
        sequencer.apply(
            Command::PlayPattern {
                pattern: 0,
                from: 0,
            },
            0,
        );
        // Step 5 has fired; step 6 has not. Runs start and end on whole blocks.
        let now = 562 * BLOCK;
        let fired = run(&mut sequencer, 0, now);
        assert_eq!(fired.last().unwrap().1, pat(0, 5));
        sequencer.apply(Command::SetLoopBar { bar: Some(1) }, now);
        let fired = run(&mut sequencer, now, 18 * 6000 - BLOCK);
        // Step 6 of bar 0 becomes step 6 of bar 1, still on the same grid.
        let expected: Vec<_> = (6..18)
            .map(|k| (block_of(k as f64 * 6000.0), pat(0, 16 + k % 16)))
            .collect();
        assert_eq!(fired, expected);
    }

    #[test]
    fn a_play_call_outside_the_loop_bar_starts_inside_it() {
        let mut sequencer = with_length(32);
        sequencer.apply(Command::SetLoopBar { bar: Some(1) }, 0);
        sequencer.apply(
            Command::PlayPattern {
                pattern: 0,
                from: 3,
            },
            0,
        );
        let fired = run(&mut sequencer, 0, 3 * 6000 - BLOCK);
        assert_eq!(steps(&fired), [19, 20, 21]);

        // The loop bar stays set across stop.
        sequencer.apply(Command::Stop, 3 * 6000);
        sequencer.apply(
            Command::PlayPattern {
                pattern: 0,
                from: 0,
            },
            3 * 6000,
        );
        let fired = run(&mut sequencer, 3 * 6000, 5 * 6000 - BLOCK);
        assert_eq!(steps(&fired), [16, 17]);
    }

    #[test]
    fn no_loop_bar_resumes_the_whole_pattern() {
        let mut sequencer = with_length(32);
        sequencer.apply(Command::SetLoopBar { bar: Some(0) }, 0);
        sequencer.apply(
            Command::PlayPattern {
                pattern: 0,
                from: 0,
            },
            0,
        );
        let now = 20 * 6000 - BLOCK;
        let fired = run(&mut sequencer, 0, now);
        let expected: Vec<_> = (0..20).map(|k| k % 16).collect();
        assert_eq!(steps(&fired), expected);
        sequencer.apply(Command::SetLoopBar { bar: None }, now);
        let fired = run(&mut sequencer, now, 50 * 6000 - BLOCK);
        let expected: Vec<_> = (4..34).map(|k| k % 32).collect();
        assert_eq!(steps(&fired), expected);
    }

    #[test]
    fn a_loop_bar_past_the_pattern_end_acts_as_none() {
        let mut sequencer = with_length(32);
        sequencer.apply(Command::SetLoopBar { bar: Some(2) }, 0);
        sequencer.apply(
            Command::PlayPattern {
                pattern: 0,
                from: 0,
            },
            0,
        );
        let now = 34 * 6000 - BLOCK;
        let fired = run(&mut sequencer, 0, now);
        let expected: Vec<_> = (0..34).map(|k| k % 32).collect();
        assert_eq!(steps(&fired), expected);

        // Bar 1 loops until the pattern gets too short to have it.
        sequencer.apply(Command::SetLoopBar { bar: Some(1) }, now);
        let later = 38 * 6000 - BLOCK;
        let fired = run(&mut sequencer, now, later);
        assert_eq!(steps(&fired), [18, 19, 20, 21]);
        sequencer.apply(
            Command::SetLength {
                pattern: 0,
                steps: 16,
            },
            later,
        );
        let fired = run(&mut sequencer, later, 58 * 6000 - BLOCK);
        let expected: Vec<_> = (6..26).map(|k| k % 16).collect();
        assert_eq!(steps(&fired), expected);
    }

    #[test]
    fn a_cued_pattern_starts_at_its_own_loop_bar() {
        let mut sequencer = with_length(32);
        sequencer.apply(
            Command::SetLength {
                pattern: 1,
                steps: 48,
            },
            0,
        );
        sequencer.apply(Command::SetLoopBar { bar: Some(1) }, 0);
        sequencer.apply(
            Command::PlayPattern {
                pattern: 0,
                from: 0,
            },
            0,
        );
        let now = 4 * 6000 - BLOCK;
        run(&mut sequencer, 0, now);
        sequencer.apply(Command::CuePattern { pattern: 1 }, now);
        let fired = run(&mut sequencer, now, 30 * 6000 - BLOCK);
        let expected: Vec<_> = (4..30)
            .map(|k| if k < 16 { pat(0, 16 + k) } else { pat(1, k) })
            .map(|p| Playhead {
                step: if p.pattern == 1 {
                    16 + p.step % 16
                } else {
                    p.step
                },
                ..p
            })
            .collect();
        let positions: Vec<_> = fired.iter().map(|&(_, p)| p).collect();
        assert_eq!(positions, expected);
    }

    #[test]
    fn song_mode_ignores_the_loop_bar_and_play_song_clears_it() {
        let mut sequencer = with_length(32);
        sequencer.apply(Command::SetLoopBar { bar: Some(1) }, 0);
        set_song(&mut sequencer, &[0], 0);
        sequencer.apply(Command::SetSongLoop { looped: true }, 0);
        sequencer.apply(Command::PlaySong { from: 0 }, 0);
        assert_eq!(sequencer.loop_bar, None);
        // Set during the song, it is kept but not used.
        sequencer.apply(Command::SetLoopBar { bar: Some(1) }, 0);
        let fired = run(&mut sequencer, 0, 40 * 6000 - BLOCK);
        let expected: Vec<_> = (0..40).map(|k| entry(0, 0, k % 32)).collect();
        let positions: Vec<_> = fired.iter().map(|&(_, p)| p).collect();
        assert_eq!(positions, expected);
    }

    #[test]
    fn the_song_plays_entries_in_order_without_drift() {
        let rate = 44_100;
        let mut sequencer = Sequencer::new(rate);
        sequencer.apply(Command::SetTempo { bpm: 133.0 }, 0);
        sequencer.apply(
            Command::SetLength {
                pattern: 4,
                steps: 32,
            },
            0,
        );
        set_song(&mut sequencer, &[0, 4, 2], 0);
        sequencer.apply(Command::SetSongLoop { looped: true }, 0);
        sequencer.apply(Command::PlaySong { from: 0 }, 0);
        let per_step = f64::from(rate) * 60.0 / (133.0 * 4.0);
        // Three times through the 64 steps of the song.
        let fired = run(&mut sequencer, 0, (192.0 * per_step) as u64 - BLOCK);
        assert_eq!(fired.len(), 192);
        for (k, &(start, position)) in fired.iter().enumerate() {
            let s = k % 64;
            let expected = match s {
                0..16 => entry(0, 0, s),
                16..48 => entry(1, 4, s - 16),
                _ => entry(2, 2, s - 48),
            };
            assert_eq!(position, expected, "step {k}");
            assert_eq!(start, block_of(k as f64 * per_step), "step {k}");
        }
    }

    #[test]
    fn a_song_that_does_not_loop_stops_after_its_last_step() {
        let mut sequencer = Sequencer::new(RATE);
        set_song(&mut sequencer, &[0, 1], 0);
        sequencer.apply(Command::PlaySong { from: 0 }, 0);
        let end = 32 * 6000;
        let fired = run(&mut sequencer, 0, end - BLOCK);
        assert_eq!(fired.len(), 32);
        assert_eq!(fired.last().unwrap().1, entry(1, 1, 15));
        assert!(
            sequencer.clock.playing,
            "still playing during the last step"
        );
        assert!(run(&mut sequencer, end - BLOCK, end + 100_000).is_empty());
        assert!(!sequencer.clock.playing);
        let history = &sequencer.history;
        assert_eq!(history.at(end - 1), Some(entry(1, 1, 15)));
        assert_eq!(history.at(end), None);
    }

    #[test]
    fn a_song_started_part_way_plays_from_that_entry() {
        let mut sequencer = Sequencer::new(RATE);
        set_song(&mut sequencer, &[0, 1, 2], 0);
        sequencer.apply(Command::PlaySong { from: 2 }, 0);
        let fired = run(&mut sequencer, 0, 40 * 6000);
        assert_eq!(fired.len(), 16);
        assert!(fired.iter().all(|(_, p)| p.song_index == Some(2)));
    }

    #[test]
    fn a_looping_song_wraps() {
        let mut sequencer = Sequencer::new(RATE);
        sequencer.apply(
            Command::SetLength {
                pattern: 1,
                steps: 4,
            },
            0,
        );
        set_song(&mut sequencer, &[1, 1], 0);
        sequencer.apply(Command::SetSongLoop { looped: true }, 0);
        sequencer.apply(Command::PlaySong { from: 0 }, 0);
        let fired = run(&mut sequencer, 0, 20 * 6000 - BLOCK);
        let positions: Vec<_> = fired.iter().map(|&(_, p)| p).collect();
        let expected: Vec<_> = (0..20).map(|k| entry((k / 4) % 2, 1, k % 4)).collect();
        assert_eq!(positions, expected);
    }

    #[test]
    fn an_empty_song_does_not_start() {
        let mut sequencer = Sequencer::new(RATE);
        sequencer.apply(Command::PlaySong { from: 0 }, 0);
        assert!(!sequencer.clock.playing);
        set_song(&mut sequencer, &[0, 1], 0);
        // An index past the end does not start either.
        sequencer.apply(Command::PlaySong { from: 2 }, 0);
        assert!(!sequencer.clock.playing);
        assert!(run(&mut sequencer, 0, 100_000).is_empty());
    }

    #[test]
    fn a_new_song_order_counts_after_the_current_entry() {
        // Entry 1 plays when the order changes; the new order still has an entry 2.
        let mut sequencer = Sequencer::new(RATE);
        set_song(&mut sequencer, &[0, 1, 2], 0);
        sequencer.apply(Command::PlaySong { from: 0 }, 0);
        run(&mut sequencer, 0, 20 * 6000);
        set_song(&mut sequencer, &[5, 6, 7], 20 * 6000);
        let fired = run(&mut sequencer, 20 * 6000, 48 * 6000);
        assert!(fired[..12].iter().all(|(_, p)| *p == entry(1, 1, p.step)));
        assert_eq!(fired[12].1, entry(2, 7, 0));

        // The new order is too short for the next index and the song loops: back to entry 0.
        let mut sequencer = Sequencer::new(RATE);
        set_song(&mut sequencer, &[0, 1, 2], 0);
        sequencer.apply(Command::SetSongLoop { looped: true }, 0);
        sequencer.apply(Command::PlaySong { from: 0 }, 0);
        run(&mut sequencer, 0, 20 * 6000);
        set_song(&mut sequencer, &[3], 20 * 6000);
        let fired = run(&mut sequencer, 20 * 6000, 36 * 6000);
        assert_eq!(fired[11].1, entry(1, 1, 15));
        assert_eq!(fired[12].1, entry(0, 3, 0));

        // The same without the loop: play stops at the end of entry 1.
        let mut sequencer = Sequencer::new(RATE);
        set_song(&mut sequencer, &[0, 1, 2], 0);
        sequencer.apply(Command::PlaySong { from: 0 }, 0);
        run(&mut sequencer, 0, 20 * 6000);
        set_song(&mut sequencer, &[3], 20 * 6000);
        let fired = run(&mut sequencer, 20 * 6000, 60 * 6000);
        assert_eq!(fired.len(), 12);
        assert_eq!(fired[11].1, entry(1, 1, 15));
        assert!(!sequencer.clock.playing);
    }

    #[test]
    fn the_playhead_reports_the_old_entry_until_it_is_heard() {
        let mut sequencer = Sequencer::new(RATE);
        sequencer.apply(
            Command::SetLength {
                pattern: 0,
                steps: 4,
            },
            0,
        );
        set_song(&mut sequencer, &[0, 2], 0);
        sequencer.apply(Command::PlaySong { from: 0 }, 0);
        run(&mut sequencer, 0, 6 * 6000);
        let change = block_of(4.0 * 6000.0);
        let history = &sequencer.history;
        assert_eq!(history.at(change), Some(entry(1, 2, 0)));
        // With 1000 frames of latency, 500 frames into entry 1 the listener still hears entry 0.
        assert_eq!(history.at(change + 500 - 1000), Some(entry(0, 0, 3)));
        assert_eq!(history.at(change + 1000 - 1000), Some(entry(1, 2, 0)));
    }

    #[test]
    fn a_step_turned_off_is_silent() {
        let (mut commands, mut consumer) = command_queue();
        let mut sequencer = Sequencer::new(RATE);
        commands.send(Command::SetRowKey { row: 0, key: 36 });
        commands.send(Command::SetRowKey { row: 1, key: 38 });
        let set = |row, step, velocity| Command::SetStep {
            pattern: 0,
            row,
            step,
            velocity,
        };
        for step in [0, 4] {
            commands.send(set(0, step, 100));
        }
        commands.send(set(1, 4, 90));
        commands.send(set(0, 4, 0));
        commands.send(Command::PlayPattern {
            pattern: 0,
            from: 0,
        });
        drain(&mut consumer, &mut sequencer, 0);

        let mut notes = Vec::new();
        let mut start = 0;
        while start < 16 * 6000 - BLOCK {
            sequencer.advance(
                start,
                BLOCK,
                |_| {},
                |key, velocity| {
                    notes.push((start, key, velocity));
                },
            );
            start += BLOCK;
        }
        assert_eq!(notes, [(0, 36, 100), (block_of(4.0 * 6000.0), 38, 90)]);
    }

    #[test]
    fn set_step_on_one_slot_leaves_the_others() {
        let mut sequencer = Sequencer::new(RATE);
        sequencer.apply(Command::SetRowKey { row: 0, key: 36 }, 0);
        sequencer.apply(
            Command::SetStep {
                pattern: 1,
                row: 0,
                step: 0,
                velocity: 100,
            },
            0,
        );
        for (pattern, expected) in [(0, 0), (1, 1), (2, 0)] {
            sequencer.apply(Command::PlayPattern { pattern, from: 0 }, 0);
            let mut notes = 0;
            sequencer.advance(0, BLOCK, |_| {}, |_, _| notes += 1);
            assert_eq!(notes, expected, "pattern {pattern}");
        }
        let on: usize = sequencer
            .patterns
            .iter()
            .map(|p| p.iter().flatten().filter(|&&v| v > 0).count())
            .sum();
        assert_eq!(on, 1);
    }

    /// Runs `sequencer` from frame `from` up to frame `to` and returns the `(step, key, velocity)`
    /// of every note it starts.
    fn notes(sequencer: &mut Sequencer, from: u64, to: u64) -> Vec<(usize, u8, u8)> {
        let mut notes = Vec::new();
        let step = std::cell::Cell::new(0);
        let mut start = from;
        while start < to {
            sequencer.advance(
                start,
                BLOCK,
                |p| step.set(p.step),
                |key, velocity| notes.push((step.get(), key, velocity)),
            );
            start += BLOCK;
        }
        notes
    }

    #[test]
    fn gain_scales_the_velocity_linearly() {
        assert_eq!(scaled_velocity(100, 1.0), 100);
        assert_eq!(scaled_velocity(127, 1.0), 127);
        assert_eq!(scaled_velocity(100, 0.5), 50);
        assert_eq!(scaled_velocity(127, 0.5), 64, "63.5 rounds up");
        assert_eq!(scaled_velocity(100, 0.0), 0);
        assert_eq!(
            scaled_velocity(100, 0.004),
            0,
            "0.4 rounds to 0, not played"
        );
        assert_eq!(scaled_velocity(100, 0.006), 1);
        assert_eq!(scaled_velocity(100, 2.0), 100, "clamped to 1.0");
        assert_eq!(scaled_velocity(100, -1.0), 0, "clamped to 0.0");
        assert_eq!(scaled_velocity(100, f32::NAN), 0);
        assert_eq!(scaled_velocity(0, 1.0), 0);
    }

    #[test]
    fn row_gain_scales_or_mutes_sequenced_notes() {
        let mut sequencer = Sequencer::new(RATE);
        for (row, key) in [(0, 36), (1, 38), (2, 42)] {
            sequencer.apply(Command::SetRowKey { row, key }, 0);
            sequencer.apply(
                Command::SetStep {
                    pattern: 0,
                    row,
                    step: 0,
                    velocity: 100,
                },
                0,
            );
        }
        sequencer.apply(Command::SetRowGain { row: 1, gain: 0.5 }, 0);
        sequencer.apply(Command::SetRowGain { row: 2, gain: 0.0 }, 0);
        sequencer.apply(
            Command::PlayPattern {
                pattern: 0,
                from: 0,
            },
            0,
        );
        assert_eq!(notes(&mut sequencer, 0, BLOCK), [(0, 36, 100), (0, 38, 50)]);
        // Gain is per row, so it holds in every pattern.
        sequencer.apply(
            Command::SetStep {
                pattern: 5,
                row: 1,
                step: 0,
                velocity: 80,
            },
            BLOCK,
        );
        sequencer.apply(
            Command::PlayPattern {
                pattern: 5,
                from: 0,
            },
            BLOCK,
        );
        assert_eq!(notes(&mut sequencer, BLOCK, 2 * BLOCK), [(0, 38, 40)]);
    }

    #[test]
    fn a_gain_change_counts_from_the_next_step() {
        let (mut commands, mut consumer) = command_queue();
        let mut sequencer = Sequencer::new(RATE);
        commands.send(Command::SetRowKey { row: 0, key: 36 });
        commands.send(Command::SetLength {
            pattern: 0,
            steps: 4,
        });
        for step in 0..4 {
            commands.send(Command::SetStep {
                pattern: 0,
                row: 0,
                step,
                velocity: 100,
            });
        }
        commands.send(Command::PlayPattern {
            pattern: 0,
            from: 0,
        });
        drain(&mut consumer, &mut sequencer, 0);
        // Into step 1, which has already started at full velocity.
        let now = block_of(6000.0) + 10 * BLOCK;
        assert_eq!(notes(&mut sequencer, 0, now), [(0, 36, 100), (1, 36, 100)]);
        commands.send(Command::SetRowGain { row: 0, gain: 0.25 });
        drain(&mut consumer, &mut sequencer, now);
        assert_eq!(
            notes(&mut sequencer, now, 4 * 6000 - BLOCK),
            [(2, 36, 25), (3, 36, 25)]
        );
        commands.send(Command::SetRowGain { row: 0, gain: 1.0 });
        drain(&mut consumer, &mut sequencer, 4 * 6000 - BLOCK);
        assert_eq!(
            notes(&mut sequencer, 4 * 6000 - BLOCK, 5 * 6000 - BLOCK),
            [(0, 36, 100)]
        );
    }

    #[test]
    fn clear_pattern_clears_one_slot_and_keeps_its_length() {
        let mut sequencer = Sequencer::new(RATE);
        for pattern in [1, 2, 3] {
            for (row, step) in [(0, 0), (5, 17), (31, 255)] {
                sequencer.apply(
                    Command::SetStep {
                        pattern,
                        row,
                        step,
                        velocity: 90,
                    },
                    0,
                );
            }
        }
        sequencer.apply(
            Command::SetLength {
                pattern: 2,
                steps: 48,
            },
            0,
        );
        sequencer.apply(Command::ClearPattern { pattern: 2 }, 0);
        let on = |p: &PatternSteps| p.iter().flatten().filter(|&&v| v > 0).count();
        assert_eq!(on(&sequencer.patterns[1]), 3);
        assert_eq!(on(&sequencer.patterns[2]), 0);
        assert_eq!(on(&sequencer.patterns[3]), 3);
        assert_eq!(sequencer.lengths[2], 48);
        // A slot out of range is ignored.
        sequencer.apply(
            Command::ClearPattern {
                pattern: MAX_PATTERNS as u8,
            },
            0,
        );
        assert_eq!(on(&sequencer.patterns[3]), 3);
    }

    #[test]
    fn clear_pattern_keeps_its_order_through_the_backlog() {
        let (mut commands, mut consumer) = command_queue();
        let mut sequencer = Sequencer::new(RATE);
        // Fill the queue so that everything after waits in the backlog.
        for i in 0..QUEUE_CAPACITY + 10 {
            commands.send(Command::SetStep {
                pattern: 4,
                row: (i % MAX_ROWS) as u8,
                step: (i / MAX_ROWS) as u8,
                velocity: 100,
            });
        }
        assert!(!commands.backlog.is_empty());
        commands.send(Command::ClearPattern { pattern: 4 });
        commands.send(Command::SetStep {
            pattern: 4,
            row: 7,
            step: 3,
            velocity: 55,
        });
        loop {
            drain(&mut consumer, &mut sequencer, 0);
            if commands.flush() {
                drain(&mut consumer, &mut sequencer, 0);
                break;
            }
        }
        let on: Vec<_> = (0..MAX_ROWS)
            .flat_map(|row| (0..MAX_STEPS).map(move |step| (row, step)))
            .filter(|&(row, step)| sequencer.patterns[4][row][step] > 0)
            .collect();
        assert_eq!(on, [(7, 3)]);
        assert_eq!(sequencer.patterns[4][7][3], 55);
    }

    #[test]
    fn a_burst_of_4000_step_changes_is_not_lost() {
        let (mut commands, mut consumer) = command_queue();
        let mut sequencer = Sequencer::new(RATE);
        for i in 0..4000 {
            commands.send(Command::SetStep {
                pattern: 3,
                row: (i / MAX_STEPS) as u8,
                step: (i % MAX_STEPS) as u8,
                velocity: 1 + (i % 127) as u8,
            });
        }
        assert!(commands.backlog.is_empty());
        drain(&mut consumer, &mut sequencer, 0);
        for i in 0..4000 {
            assert_eq!(
                sequencer.patterns[3][i / MAX_STEPS][i % MAX_STEPS],
                1 + (i % 127) as u8
            );
        }
    }

    #[test]
    fn commands_beyond_the_queue_wait_and_keep_their_order() {
        let (mut commands, mut consumer) = command_queue();
        let mut sequencer = Sequencer::new(RATE);
        let total = 3 * QUEUE_CAPACITY + 7;
        for i in 0..total {
            commands.send(Command::SetStep {
                pattern: 0,
                row: (i % MAX_ROWS) as u8,
                step: 5,
                velocity: (i % 128) as u8,
            });
        }
        assert!(!commands.backlog.is_empty());
        // A song and a play sent behind the backlog arrive after it, in order.
        commands.set_song(&[7, 8]);
        commands.send(Command::PlaySong { from: 1 });
        // A hit is dropped while the queue is full, rather than held.
        commands.hit(36, 100);
        let mut rounds = 0;
        loop {
            drain(&mut consumer, &mut sequencer, 0);
            if commands.flush() {
                drain(&mut consumer, &mut sequencer, 0);
                break;
            }
            rounds += 1;
        }
        assert_eq!(rounds, 2);
        for row in 0..MAX_ROWS {
            let last = (0..total).filter(|i| i % MAX_ROWS == row).max().unwrap();
            assert_eq!(sequencer.patterns[0][row][5], (last % 128) as u8);
        }
        assert_eq!(sequencer.next_position(), Some(entry(1, 8, 0)));
    }

    #[test]
    fn the_song_order_is_truncated_at_max_song() {
        let (mut commands, mut consumer) = command_queue();
        let mut sequencer = Sequencer::new(RATE);
        let order: Vec<_> = (0..MAX_SONG + 10).map(|i| i % 40).collect();
        assert_eq!(commands.set_song(&order), MAX_SONG);
        drain(&mut consumer, &mut sequencer, 0);
        assert_eq!(sequencer.song_len, MAX_SONG);
        assert_eq!(sequencer.song[3], 3);
        assert_eq!(usize::from(sequencer.song[30]), MAX_PATTERNS - 1);
    }

    #[test]
    fn the_playhead_allows_for_latency() {
        let mut sequencer = Sequencer::new(RATE);
        assert_eq!(sequencer.history.at(0), None);
        sequencer.apply(
            Command::PlayPattern {
                pattern: 0,
                from: 0,
            },
            0,
        );
        let mut start = 0;
        while start < 3 * 6000 - BLOCK {
            sequencer.advance(start, BLOCK, |_| {}, |_, _| {});
            start += BLOCK;
        }
        let step1 = block_of(6000.0);
        let step2 = block_of(12_000.0);
        let history = &sequencer.history;
        assert_eq!(history.at(step1 - 1), Some(pat(0, 0)));
        assert_eq!(history.at(step1), Some(pat(0, 1)));
        assert_eq!(history.at(step2 + 500), Some(pat(0, 2)));
        // With 1000 frames of latency, the listener hears step 1 until 1000 frames into step 2.
        assert_eq!(history.at(step2 + 500 - 1000), Some(pat(0, 1)));

        sequencer.apply(Command::Stop, start);
        assert_eq!(sequencer.history.at(start), None);
        assert_eq!(sequencer.history.at(start - 1), Some(pat(0, 2)));
    }

    #[test]
    fn the_playhead_packs_into_one_word() {
        for playhead in [
            None,
            Some(pat(0, 0)),
            Some(pat(MAX_PATTERNS - 1, MAX_STEPS - 1)),
            Some(entry(MAX_SONG - 1, 7, 200)),
            Some(entry(0, 0, 0)),
        ] {
            assert_eq!(Playhead::unpack(Playhead::pack(playhead)), playhead);
        }
    }

    #[test]
    fn silence_renders_zeros() {
        let font = soundfont_or_skip!();
        let (_commands, mut renderer) = Renderer::new(&font, RATE).unwrap();
        let mut out = vec![1.0_f32; 2 * 4_800];
        renderer.render(&mut out, 2);
        assert!(out.iter().all(|&s| s == 0.0));
    }

    #[test]
    fn a_kick_has_energy_in_every_format() {
        let font = soundfont_or_skip!();
        let frames = (RATE / 10) as usize;

        let (mut commands, mut renderer) = Renderer::new(&font, RATE).unwrap();
        commands.hit(36, 127);
        // An odd buffer length checks that leftover frames carry over between calls.
        let mut out = vec![0.0_f32; 2 * frames];
        for chunk in out.chunks_mut(2 * 37) {
            renderer.render(chunk, 2);
        }
        let rms = energy(&out);
        assert!(rms > 1e-3, "kick RMS {rms}");

        let (mut commands, mut renderer) = Renderer::new(&font, RATE).unwrap();
        commands.hit(36, 127);
        let mut mono = vec![0_i16; frames];
        renderer.render(&mut mono, 1);
        assert!(mono.iter().any(|&s| s.unsigned_abs() > 100));

        let (mut commands, mut renderer) = Renderer::new(&font, RATE).unwrap();
        commands.hit(36, 127);
        let mut quad = vec![0.0_f32; 4 * frames];
        renderer.render(&mut quad, 4);
        assert!(quad.chunks(4).all(|f| f[2] == 0.0 && f[3] == 0.0));
        assert!(energy(&quad) > 1e-4);
    }

    #[test]
    fn a_kit_change_does_not_panic() {
        let font = soundfont_or_skip!();
        let (mut commands, mut renderer) = Renderer::new(&font, RATE).unwrap();
        let mut out = vec![0.0_f32; 2 * 4_800];
        for program in [0, 8, 16, 25, 32, 40, 48, 56, 127] {
            commands.set_kit(program);
            commands.hit(38, 100);
            renderer.render(&mut out, 2);
        }
        assert!(energy(&out) > 0.0);
    }

    #[test]
    fn drum_kits_lists_the_percussion_bank() {
        let font = soundfont_or_skip!();
        let kits = drum_kits(&font);
        assert!(!kits.is_empty());
        assert_eq!(kits[0].program, 0);
        assert!(kits.windows(2).all(|w| w[0].program < w[1].program));
        assert!(kits.iter().all(|k| k.name == k.name.trim()));
        let presets = font
            .get_presets()
            .iter()
            .filter(|p| p.get_bank_number() == PERCUSSION_BANK)
            .count();
        println!("{} kits from {presets} presets in bank 128:", kits.len());
        for kit in &kits {
            println!("{:3} {}", kit.program, kit.name);
        }
    }

    /// A kit list made by hand, one kit per `(program, name)`.
    fn kit_list(kits: &[(u8, &str)]) -> Vec<Kit> {
        kits.iter()
            .map(|&(program, name)| Kit {
                program,
                name: name.to_string(),
            })
            .collect()
    }

    #[test]
    fn resolve_kit_prefers_the_same_program() {
        let kits = kit_list(&[(0, "Standard"), (25, "TR-808"), (26, "Swirls")]);
        // The program wins over a name that matches another kit.
        assert_eq!(resolve_kit(&kits, 25, Some("Swirls")), Some(25));
        assert_eq!(resolve_kit(&kits, 26, None), Some(26));
    }

    #[test]
    fn resolve_kit_then_matches_the_name() {
        let kits = kit_list(&[(0, "Standard"), (24, "Electronic"), (40, "Brush  Kit")]);
        assert_eq!(resolve_kit(&kits, 25, Some("Electronic")), Some(24));
        // Case and whitespace, inside and around the name, do not matter.
        assert_eq!(resolve_kit(&kits, 41, Some(" brush kit ")), Some(40));
        assert_eq!(resolve_kit(&kits, 41, Some("BRUSHKIT")), Some(40));
        // Other characters do.
        assert_eq!(resolve_kit(&kits, 41, Some("Brush-Kit")), Some(40));
        let kits = kit_list(&[(0, "Standard"), (50, "Brush Kit")]);
        assert_eq!(resolve_kit(&kits, 41, Some("Brush-Kit")), Some(0));
    }

    #[test]
    fn resolve_kit_ignores_an_empty_name() {
        let kits = kit_list(&[(0, "Standard"), (9, "")]);
        assert_eq!(resolve_kit(&kits, 30, Some("  ")), Some(0));
    }

    #[test]
    fn resolve_kit_then_takes_the_first_kit_of_the_group() {
        let kits = kit_list(&[(0, "Standard"), (24, "Electronic"), (27, "Dance")]);
        // 25 and 31 are in the group 24 to 31.
        assert_eq!(resolve_kit(&kits, 25, Some("TR-808")), Some(24));
        assert_eq!(resolve_kit(&kits, 31, None), Some(24));
        // The group's first program must be present itself; another kit of the group is not
        // enough.
        let kits = kit_list(&[(0, "Standard"), (27, "Dance")]);
        assert_eq!(resolve_kit(&kits, 25, None), Some(0));
    }

    #[test]
    fn resolve_kit_then_takes_standard() {
        let kits = kit_list(&[(0, "Standard"), (8, "Room"), (16, "Power")]);
        assert_eq!(resolve_kit(&kits, 25, Some("TR-808")), Some(0));
        assert_eq!(resolve_kit(&kits, 127, None), Some(0));
    }

    #[test]
    fn resolve_kit_then_takes_the_first_kit() {
        let kits = kit_list(&[(8, "Room"), (16, "Power")]);
        assert_eq!(resolve_kit(&kits, 25, Some("TR-808")), Some(8));
        assert_eq!(resolve_kit(&kits, 0, None), Some(8));
    }

    #[test]
    fn resolve_kit_of_no_kits_is_none() {
        assert_eq!(resolve_kit(&[], 0, Some("Standard")), None);
    }

    #[test]
    fn kit_keys_of_the_standard_kit() {
        let font = soundfont_or_skip!();
        let keys = kit_keys(&font, 0);
        // The General MIDI drums, kick 36 to open triangle 81, all have a sound.
        assert!((35..=81).all(|key| keys[key]), "{keys:?}");
        let listed: Vec<usize> = (0..128).filter(|&key| keys[key]).collect();
        println!("128:0 has {} keys: {listed:?}", listed.len());
    }

    #[test]
    fn kit_keys_of_a_missing_kit_is_empty() {
        let font = soundfont_or_skip!();
        let kits = drum_kits(&font);
        let missing = (0..=127_u8)
            .find(|&p| kits.iter().all(|k| k.program != p))
            .expect("the bank has a free program in bank 128");
        assert!(kit_keys(&font, missing).iter().all(|&key| !key));
    }

    #[test]
    fn kit_keys_match_what_sounds() {
        let font = soundfont_or_skip!();
        let keys = kit_keys(&font, 0);
        // A key with a sound renders some, and one without renders silence.
        let render = |key: u8| {
            let (mut commands, mut renderer) = Renderer::new(&font, RATE).unwrap();
            commands.set_kit(0);
            commands.hit(key, 100);
            let mut out = vec![0.0_f32; 2 * 4_800];
            renderer.render(&mut out, 2);
            energy(&out)
        };
        let with = (0..128).find(|&k| keys[k as usize]).unwrap() as u8;
        assert!(render(with) > 0.0, "key {with}");
        if let Some(without) = (0..128).find(|&k| !keys[k as usize]) {
            assert_eq!(render(without as u8), 0.0, "key {without}");
        }
    }

    /// Renders 0.1 s of a snare under kit `program`: a preview hit, or a sequenced note if
    /// `sequenced`.
    fn snare(font: &Arc<SoundFont>, program: u8, sequenced: bool) -> Vec<f32> {
        let (mut commands, mut renderer) = Renderer::new(font, RATE).unwrap();
        commands.set_kit(program);
        if sequenced {
            commands.send(Command::SetRowKey { row: 0, key: 38 });
            commands.send(Command::SetStep {
                pattern: 0,
                row: 0,
                step: 0,
                velocity: 100,
            });
            commands.send(Command::PlayPattern {
                pattern: 0,
                from: 0,
            });
        } else {
            commands.hit(38, 100);
        }
        let mut out = vec![0.0_f32; 2 * 4_800];
        renderer.render(&mut out, 2);
        out
    }

    #[test]
    fn a_kit_changes_previews_and_sequenced_notes() {
        let font = soundfont_or_skip!();
        let kits = drum_kits(&font);
        // A kit whose snare sounds different from the standard kit's.
        let standard = snare(&font, 0, false);
        let Some(other) = kits
            .iter()
            .map(|k| k.program)
            .find(|&p| p != 0 && snare(&font, p, false) != standard)
        else {
            panic!("every kit of the bank has the standard snare");
        };
        for sequenced in [false, true] {
            let a = snare(&font, 0, sequenced);
            let b = snare(&font, other, sequenced);
            assert!(energy(&a) > 1e-3 && energy(&b) > 1e-3);
            assert_ne!(a, b, "kit {other}, sequenced {sequenced}");
        }
        // A program the bank lacks falls back to the standard kit, 128:0.
        if let Some(missing) = (0..=127).find(|p| kits.iter().all(|k| k.program != *p)) {
            for sequenced in [false, true] {
                assert_eq!(
                    snare(&font, missing, sequenced),
                    snare(&font, 0, sequenced),
                    "program {missing}, sequenced {sequenced}"
                );
            }
        }
    }

    #[test]
    fn note_off_follows_the_last_hit_on_a_key() {
        let font = soundfont_or_skip!();
        let (mut commands, mut renderer) = Renderer::new(&font, RATE).unwrap();
        let mut out = vec![0.0_f32; 2 * (RATE as usize)];
        commands.hit(42, 100);
        renderer.render(&mut out, 2);
        let first_due = renderer.note_off_at[42];
        assert_ne!(first_due, NO_NOTE_OFF);
        commands.hit(42, 100);
        renderer.render(&mut out, 2);
        assert!(renderer.note_off_at[42] > first_due);
        for _ in 0..(RING_SECONDS as usize + 1) {
            renderer.render(&mut out, 2);
        }
        assert_eq!(renderer.note_off_at[42], NO_NOTE_OFF);
    }

    #[test]
    fn silence_fades_every_sound_to_nothing() {
        let font = soundfont_or_skip!();
        let (mut commands, mut renderer) = Renderer::new(&font, RATE).unwrap();
        // A crash cymbal rings for seconds.
        commands.hit(49, 127);
        let mut out = vec![0.0_f32; 2 * (RATE as usize / 10)];
        renderer.render(&mut out, 2);
        assert!(energy(&out) > 1e-3);

        commands.send(Command::Silence);
        let fade = renderer.fade_frames as usize;
        let mut faded = vec![0.0_f32; 2 * fade];
        renderer.render(&mut faded, 2);
        // The fade starts from the sound and falls to nothing, without a jump.
        assert!(energy(&faded[..fade / 2]) > 1e-4);
        let peak_end = faded[2 * fade - 2 * 64..]
            .iter()
            .fold(0.0_f32, |m, s| m.max(s.abs()));
        let peak_start = faded[..2 * 64].iter().fold(0.0_f32, |m, s| m.max(s.abs()));
        assert!(peak_end < peak_start / 4.0, "{peak_end} vs {peak_start}");

        // After the fade, nothing is left: no voice, no reverb tail, no pending note-off.
        renderer.render(&mut out, 2);
        assert!(out.iter().all(|s| *s == 0.0));
        assert_eq!(renderer.synth.get_active_voice_count(), 0);
        assert_eq!(renderer.note_off_at[49], NO_NOTE_OFF);
        assert_eq!(renderer.fade_left, 0);
    }

    #[test]
    fn a_hit_during_the_fade_is_heard_whole() {
        let font = soundfont_or_skip!();
        let (mut commands, mut renderer) = Renderer::new(&font, RATE).unwrap();
        let mut fresh = vec![0.0_f32; 2 * (RATE as usize / 10)];
        commands.hit(38, 100);
        renderer.render(&mut fresh, 2);

        let (mut commands, mut renderer) = Renderer::new(&font, RATE).unwrap();
        commands.hit(49, 127);
        let mut out = vec![0.0_f32; 2 * (RATE as usize / 10)];
        renderer.render(&mut out, 2);
        commands.send(Command::Silence);
        let mut block = vec![0.0_f32; 2 * 64];
        renderer.render(&mut block, 2);
        assert!(renderer.fade_left > 0);
        commands.hit(38, 100);
        renderer.render(&mut out, 2);
        assert_eq!(renderer.fade_left, 0);
        // The snare sounds as it does on a fresh synthesizer: the cymbal and the fade are gone.
        assert_eq!(out, fresh);
    }

    #[test]
    fn silence_keeps_the_kit() {
        let font = soundfont_or_skip!();
        // A kit whose snare differs from the standard kit's, which a reset would fall back to.
        let standard = snare(&font, 0, false);
        let Some(other) = drum_kits(&font)
            .iter()
            .map(|k| k.program)
            .find(|&p| p != 0 && snare(&font, p, false) != standard)
        else {
            panic!("every kit of the bank has the standard snare");
        };
        let played = |silence: bool| {
            let (mut commands, mut renderer) = Renderer::new(&font, RATE).unwrap();
            commands.send(Command::ProgramChange { program: other });
            if silence {
                commands.send(Command::Silence);
                let mut fade = vec![0.0_f32; 2 * RATE as usize / 10];
                renderer.render(&mut fade, 2);
            }
            commands.hit(38, 100);
            let mut out = vec![0.0_f32; 2 * RATE as usize / 10];
            renderer.render(&mut out, 2);
            out
        };
        assert_eq!(played(true), played(false));
    }

    #[test]
    fn a_full_queue_drops_hits() {
        let font = soundfont_or_skip!();
        let (mut commands, mut renderer) = Renderer::new(&font, RATE).unwrap();
        for _ in 0..(2 * QUEUE_CAPACITY) {
            commands.hit(36, 1);
        }
        assert!(commands.backlog.is_empty());
        let mut out = vec![0.0_f32; 2 * 64];
        renderer.render(&mut out, 2);
        assert!(commands.producer.slots() == QUEUE_CAPACITY);
    }

    /// The playhead the renderer last published.
    fn published(renderer: &Renderer) -> Option<Playhead> {
        Playhead::unpack(renderer.status.playhead.load(Ordering::Relaxed))
    }

    #[test]
    fn the_pattern_plays_with_hits_and_note_offs() {
        let font = soundfont_or_skip!();
        let (mut commands, mut renderer) = Renderer::new(&font, RATE).unwrap();
        commands.send(Command::SetRowKey { row: 0, key: 36 });
        commands.send(Command::SetStep {
            pattern: 0,
            row: 0,
            step: 0,
            velocity: 127,
        });
        commands.send(Command::SetLength {
            pattern: 0,
            steps: 4,
        });
        commands.send(Command::PlayPattern {
            pattern: 0,
            from: 0,
        });
        // Most of step 0: the kick sounds and step 1 has not started.
        let mut out = vec![0.0_f32; 2 * 5900];
        renderer.render(&mut out, 2);
        assert!(energy(&out) > 1e-3);
        assert_eq!(published(&renderer), Some(pat(0, 0)));
        assert!(renderer.status.playing.load(Ordering::Relaxed));
        assert_eq!(renderer.status.transport.load(Ordering::Relaxed), 1);
        assert_eq!(renderer.note_off_at[36], renderer.ring_frames);

        // A preview hit while playing is heard too and gets its own note-off deadline.
        commands.hit(49, 120);
        renderer.render(&mut out, 2);
        assert_eq!(published(&renderer), Some(pat(0, 1)));
        assert_ne!(renderer.note_off_at[49], NO_NOTE_OFF);

        // After a stop, nothing new starts, so every note-off comes due.
        commands.send(Command::Stop);
        let mut long = vec![0.0_f32; 2 * (RATE as usize)];
        for _ in 0..(RING_SECONDS as usize + 1) {
            renderer.render(&mut long, 2);
        }
        assert_eq!(renderer.note_off_at[36], NO_NOTE_OFF);
        assert_eq!(renderer.note_off_at[49], NO_NOTE_OFF);
        assert_eq!(published(&renderer), None);
        assert!(!renderer.status.playing.load(Ordering::Relaxed));
        assert_eq!(renderer.status.transport.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn a_song_that_does_not_loop_ends_in_the_renderer() {
        let font = soundfont_or_skip!();
        let (mut commands, mut renderer) = Renderer::new(&font, RATE).unwrap();
        renderer.set_latency(960);
        commands.send(Command::SetRowKey { row: 0, key: 36 });
        commands.send(Command::SetStep {
            pattern: 1,
            row: 0,
            step: 0,
            velocity: 127,
        });
        commands.set_song(&[1, 1]);
        commands.send(Command::PlaySong { from: 0 });
        // 31 steps in: the last step of entry 1 is playing and heard.
        let mut out = vec![0.0_f32; 2 * (31 * 6000 + 2000)];
        renderer.render(&mut out, 2);
        assert!(energy(&out) > 1e-3);
        assert_eq!(published(&renderer), Some(entry(1, 1, 15)));
        assert!(renderer.status.playing.load(Ordering::Relaxed));
        // Past the end of the last step, by less than the latency: no longer playing, but the
        // last step is still what the listener hears.
        let mut out = vec![0.0_f32; 2 * (6000 - 2000 + 500)];
        renderer.render(&mut out, 2);
        assert!(!renderer.status.playing.load(Ordering::Relaxed));
        assert_eq!(published(&renderer), Some(entry(1, 1, 15)));
        // Once the latency has passed, the playhead is gone too.
        let mut out = vec![0.0_f32; 2 * 1000];
        renderer.render(&mut out, 2);
        assert_eq!(published(&renderer), None);
    }

    /// Every MIDI message queued so far, oldest first.
    fn midi_sent(events: &mut MidiEvents) -> Vec<MidiEvent> {
        std::iter::from_fn(|| events.pop().ok()).collect()
    }

    /// The bytes of each message.
    fn messages(events: &[MidiEvent]) -> Vec<Vec<u8>> {
        events.iter().map(|e| e.message().to_vec()).collect()
    }

    /// The bytes of each message that is not a note-off.
    fn without_note_offs(events: &[MidiEvent]) -> Vec<Vec<u8>> {
        messages(events)
            .into_iter()
            .filter(|m| m[0] != MIDI_NOTE_OFF)
            .collect()
    }

    /// Checks that no key gets a second note-on before a note-off.
    fn assert_note_offs_between(events: &[MidiEvent]) {
        let mut sounding = [false; 128];
        for (i, event) in events.iter().enumerate() {
            let message = event.message();
            let key = usize::from(message.get(1).copied().unwrap_or(0));
            match message[0] {
                MIDI_NOTE_ON => {
                    assert!(
                        !sounding[key],
                        "message {i}: note-on for sounding key {key}"
                    );
                    sounding[key] = true;
                }
                MIDI_NOTE_OFF => sounding[key] = false,
                _ => {}
            }
        }
    }

    /// How long MIDI out lets a note sound, as the renderer counts it.
    fn midi_note(renderer: &Renderer) -> Duration {
        Duration::from_secs_f64(renderer.midi_note_frames as f64 / f64::from(RATE))
    }

    /// A renderer with the MIDI queue taken out, row 0 on `key` and pattern 0 four steps long.
    fn midi_renderer(font: &Arc<SoundFont>, key: u8) -> (Commands, Renderer, MidiEvents) {
        let (mut commands, mut renderer) = Renderer::new(font, RATE).unwrap();
        let events = renderer.take_midi_events().unwrap();
        assert!(renderer.take_midi_events().is_none());
        commands.send(Command::SetRowKey { row: 0, key });
        commands.send(Command::SetLength {
            pattern: 0,
            steps: 4,
        });
        (commands, renderer, events)
    }

    #[test]
    fn midi_out_sends_steps_and_silences_the_synthesizer() {
        let font = soundfont_or_skip!();
        let (mut commands, mut renderer, mut events) = midi_renderer(&font, 36);
        renderer.set_latency(960);
        for (step, velocity) in [(0, 100), (1, 90), (3, 80)] {
            commands.send(Command::step(0, 0, step, velocity).unwrap());
        }
        commands.send(Command::SetMidiOut { on: true });
        commands.send(Command::PlayPattern {
            pattern: 0,
            from: 0,
        });
        // Two loops of the pattern in one call, so the message times share one start moment.
        let mut out = vec![1.0_f32; 2 * 8 * 6000];
        let before = Instant::now();
        renderer.render(&mut out, 2);
        let after = Instant::now();
        assert!(out.iter().all(|&s| s == 0.0));

        let sent = midi_sent(&mut events);
        // Each note ends before the next step, which comes later than the MIDI note length.
        let velocities = [100, 90, 80, 100, 90, 80];
        let mut expected = vec![vec![0xC9, 0], vec![0xB9, 7, 127]];
        for &v in &velocities {
            expected.push(vec![0x99, 36, v]);
            expected.push(vec![0x89, 36, 0]);
        }
        assert_eq!(messages(&sent), expected);

        // The first message is due when the first block is heard, after the latency.
        let latency = Duration::from_secs_f64(960.0 / f64::from(RATE));
        assert!(sent[0].at >= before + latency && sent[0].at <= after + latency);
        assert_eq!(sent[1].at, sent[0].at);
        assert_eq!(sent[2].at, sent[0].at);
        // Each step is due at the start of the block it fires in: the block holding its frame.
        // Its note-off follows one MIDI note length later.
        let first = sent[0].at;
        let note = midi_note(&renderer);
        for (pair, step) in sent[2..].chunks(2).zip([0_u64, 1, 3, 4, 5, 7]) {
            let frame = step * 6000 / BLOCK * BLOCK;
            let expected = first + Duration::from_secs_f64(frame as f64 / f64::from(RATE));
            let error = pair[0].at.max(expected) - pair[0].at.min(expected);
            assert!(error < Duration::from_micros(1), "step {step}: {error:?}");
            let error = (pair[1].at - pair[0].at).abs_diff(note);
            assert!(
                error < Duration::from_micros(1),
                "step {step} note-off: {error:?}"
            );
        }
    }

    #[test]
    fn midi_out_sends_note_offs_after_the_midi_note_length() {
        let font = soundfont_or_skip!();
        let (mut commands, mut renderer, mut events) = midi_renderer(&font, 36);
        commands.send(Command::SetMidiOut { on: true });
        commands.hit(42, 100);
        let mut out = vec![1.0_f32; 2 * (RING_SECONDS as usize + 1) * RATE as usize];
        renderer.render(&mut out, 2);
        assert!(out.iter().all(|&s| s == 0.0));
        let sent = midi_sent(&mut events);
        assert_eq!(
            messages(&sent),
            [
                vec![0xC9, 0],
                vec![0xB9, 7, 127],
                vec![0x99, 42, 100],
                vec![0x89, 42, 0]
            ]
        );
        let error = (sent[3].at - sent[2].at).abs_diff(midi_note(&renderer));
        assert!(error < Duration::from_micros(1), "{error:?}");
        // The synthesizer's own note-off still waits for the whole ring.
        assert!(renderer.midi_note_frames < renderer.ring_frames);
    }

    #[test]
    fn midi_out_ends_a_note_before_the_same_key_sounds_again() {
        let font = soundfont_or_skip!();
        let (mut commands, mut renderer, mut events) = midi_renderer(&font, 36);
        commands.send(Command::SetMidiOut { on: true });
        commands.hit(42, 100);
        let mut out = vec![0.0_f32; 2 * 640];
        renderer.render(&mut out, 2);
        // Sooner than the MIDI note length: the first note is still sounding.
        commands.hit(42, 90);
        renderer.render(&mut out, 2);
        let sent = midi_sent(&mut events);
        assert_eq!(
            messages(&sent),
            [
                vec![0xC9, 0],
                vec![0xB9, 7, 127],
                vec![0x99, 42, 100],
                vec![0x89, 42, 0],
                vec![0x99, 42, 90],
            ]
        );
        assert_eq!(sent[3].at, sent[4].at);
        assert_note_offs_between(&sent);
    }

    #[test]
    fn a_steady_beat_keeps_ending_its_midi_notes() {
        let font = soundfont_or_skip!();
        let (mut commands, mut renderer, mut events) = midi_renderer(&font, 42);
        // A hi-hat on every step, sooner than the synthesizer's ring. Before the fix, MIDI out
        // sent no note-off at all while this played.
        for step in 0..4 {
            commands.send(Command::step(0, 0, step, 80).unwrap());
        }
        commands.send(Command::SetMidiOut { on: true });
        commands.send(Command::PlayPattern {
            pattern: 0,
            from: 0,
        });
        let mut out = vec![0.0_f32; 2 * 16 * 6000];
        renderer.render(&mut out, 2);
        let sent = midi_sent(&mut events);
        let ons = sent
            .iter()
            .filter(|e| e.message()[0] == MIDI_NOTE_ON)
            .count();
        let offs = sent
            .iter()
            .filter(|e| e.message()[0] == MIDI_NOTE_OFF)
            .count();
        assert_eq!(ons, 16);
        assert_eq!(offs, 16);
        assert_note_offs_between(&sent);
    }

    #[test]
    fn midi_out_sends_the_kit_on_enable_and_on_change() {
        let font = soundfont_or_skip!();
        let (mut commands, mut renderer, mut events) = midi_renderer(&font, 36);
        let mut out = vec![0.0_f32; 2 * 640];
        // A kit chosen while MIDI out is off is sent when it is turned on.
        commands.set_kit(8);
        renderer.render(&mut out, 2);
        assert!(midi_sent(&mut events).is_empty());
        commands.send(Command::SetMidiOut { on: true });
        renderer.render(&mut out, 2);
        commands.set_kit(16);
        renderer.render(&mut out, 2);
        assert_eq!(
            messages(&midi_sent(&mut events)),
            [vec![0xC9, 8], vec![0xB9, 7, 127], vec![0xC9, 16]]
        );
    }

    #[test]
    fn midi_out_sends_the_volume_on_enable_and_on_change() {
        let font = soundfont_or_skip!();
        let (mut commands, mut renderer, mut events) = midi_renderer(&font, 36);
        let mut out = vec![0.0_f32; 2 * 640];
        // A volume set while MIDI out is off is kept and sent when it is turned on.
        commands.send(Command::SetMidiVolume { volume: 90 });
        renderer.render(&mut out, 2);
        assert!(midi_sent(&mut events).is_empty());
        commands.send(Command::SetMidiOut { on: true });
        renderer.render(&mut out, 2);
        commands.send(Command::SetMidiVolume { volume: 0 });
        renderer.render(&mut out, 2);
        assert_eq!(
            messages(&midi_sent(&mut events)),
            [vec![0xC9, 0], vec![0xB9, 7, 90], vec![0xB9, 7, 0]]
        );
        // Off and on again sends it again.
        commands.send(Command::SetMidiOut { on: false });
        commands.send(Command::SetMidiOut { on: true });
        renderer.render(&mut out, 2);
        assert_eq!(
            messages(&midi_sent(&mut events)),
            [vec![0xC9, 0], vec![0xB9, 7, 0]]
        );
    }

    #[test]
    fn the_midi_volume_leaves_the_synthesizer_alone() {
        let font = soundfont_or_skip!();
        let loud = {
            let (mut commands, mut renderer, _events) = midi_renderer(&font, 36);
            commands.hit(38, 100);
            let mut out = vec![0.0_f32; 2 * 6000];
            renderer.render(&mut out, 2);
            out
        };
        let (mut commands, mut renderer, _events) = midi_renderer(&font, 36);
        commands.send(Command::SetMidiVolume { volume: 0 });
        commands.hit(38, 100);
        let mut out = vec![0.0_f32; 2 * 6000];
        renderer.render(&mut out, 2);
        assert!(energy(&loud) > 1e-3);
        assert_eq!(out, loud);
    }

    #[test]
    fn the_volume_scales_the_output_after_a_ramp() {
        let font = soundfont_or_skip!();
        let full = {
            let (mut commands, mut renderer, _events) = midi_renderer(&font, 36);
            commands.hit(38, 100);
            let mut out = vec![0.0_f32; 2 * 6000];
            renderer.render(&mut out, 2);
            out
        };
        let (mut commands, mut renderer, _events) = midi_renderer(&font, 36);
        commands.send(Command::SetVolume { gain: 0.25 });
        commands.hit(38, 100);
        let mut out = vec![0.0_f32; 2 * 6000];
        renderer.render(&mut out, 2);
        let block = renderer.left.len();
        assert!(energy(&full) > 1e-3);
        // After the first block, every sample is the full one times the gain.
        for (quiet, loud) in out[2 * block..].iter().zip(&full[2 * block..]) {
            assert!((quiet - loud * 0.25).abs() < 1e-6);
        }
        // During the first block the gain falls from 1.0 to 0.25 and never jumps.
        for frame in 0..block {
            let gain = 1.0 - 0.75 * ((frame + 1) as f32 / block as f32);
            assert!((out[2 * frame] - full[2 * frame] * gain).abs() < 1e-6);
        }
    }

    #[test]
    fn volume_zero_is_silence_and_nan_is_full() {
        let font = soundfont_or_skip!();
        let (mut commands, mut renderer, _events) = midi_renderer(&font, 36);
        commands.send(Command::SetVolume { gain: 0.0 });
        let mut out = vec![0.0_f32; 2 * 640];
        renderer.render(&mut out, 2);
        commands.hit(38, 100);
        renderer.render(&mut out, 2);
        assert_eq!(energy(&out), 0.0);
        assert_eq!(clamp_gain(f32::NAN), 1.0);
        assert_eq!(clamp_gain(2.0), 1.0);
        assert_eq!(clamp_gain(-1.0), 0.0);
    }

    #[test]
    fn midi_out_off_sends_nothing_and_plays_the_synthesizer() {
        let font = soundfont_or_skip!();
        let (mut commands, mut renderer, mut events) = midi_renderer(&font, 36);
        commands.send(Command::step(0, 0, 0, 127).unwrap());
        commands.send(Command::PlayPattern {
            pattern: 0,
            from: 0,
        });
        commands.set_kit(8);
        commands.hit(38, 100);
        let mut out = vec![0.0_f32; 2 * 4 * 6000];
        renderer.render(&mut out, 2);
        assert!(energy(&out) > 1e-3);
        assert!(midi_sent(&mut events).is_empty());
    }

    #[test]
    fn turning_midi_out_off_ends_sounding_notes_and_brings_the_sound_back() {
        let font = soundfont_or_skip!();
        let (mut commands, mut renderer, mut events) = midi_renderer(&font, 36);
        commands.send(Command::SetMidiOut { on: true });
        commands.hit(42, 100);
        commands.hit(49, 100);
        // Shorter than the MIDI note length, so both notes still sound.
        let mut out = vec![0.0_f32; 2 * 2400];
        renderer.render(&mut out, 2);
        assert!(out.iter().all(|&s| s == 0.0));
        // The program change, the volume and the two notes.
        assert_eq!(midi_sent(&mut events).len(), 4);

        commands.send(Command::SetMidiOut { on: false });
        commands.hit(38, 100);
        renderer.render(&mut out, 2);
        assert_eq!(
            messages(&midi_sent(&mut events)),
            [[0x89, 42, 0], [0x89, 49, 0]]
        );
        assert!(energy(&out) > 1e-3);
        // Later note-offs and hits send nothing.
        let mut long = vec![0.0_f32; 2 * (RING_SECONDS as usize + 1) * RATE as usize];
        renderer.render(&mut long, 2);
        assert_eq!(renderer.note_off_at[42], NO_NOTE_OFF);
        assert!(midi_sent(&mut events).is_empty());
    }

    #[test]
    fn a_full_midi_queue_drops_messages() {
        let font = soundfont_or_skip!();
        let (mut commands, mut renderer, mut events) = midi_renderer(&font, 36);
        commands.send(Command::SetMidiOut { on: true });
        let mut out = vec![0.0_f32; 2 * 64];
        for _ in 0..(MIDI_QUEUE_CAPACITY / QUEUE_CAPACITY + 1) {
            for _ in 0..QUEUE_CAPACITY {
                commands.hit(36, 100);
            }
            renderer.render(&mut out, 2);
        }
        assert_eq!(midi_sent(&mut events).len(), MIDI_QUEUE_CAPACITY);
    }

    fn hit(step: usize, key: u8, velocity: u8) -> PreviewHit {
        PreviewHit {
            step,
            key,
            velocity,
        }
    }

    /// Sends a preview through the command queue at frame `now`, like the engine does.
    fn preview(sequencer: &mut Sequencer, hits: &[PreviewHit], steps: usize, now: u64) {
        let (mut commands, mut consumer) = command_queue();
        for command in Command::preview(hits, steps) {
            commands.send(command);
        }
        drain(&mut consumer, sequencer, now);
    }

    /// Runs `sequencer` from frame `from` up to frame `to` and returns the `(block start, key,
    /// velocity)` of every note it starts.
    fn timed_notes(sequencer: &mut Sequencer, from: u64, to: u64) -> Vec<(u64, u8, u8)> {
        let mut notes = Vec::new();
        let mut start = from;
        while start < to {
            sequencer.advance(start, BLOCK, |_| {}, |k, v| notes.push((start, k, v)));
            start += BLOCK;
        }
        notes
    }

    #[test]
    fn a_preview_plays_its_hits_on_time_and_loops() {
        let mut sequencer = Sequencer::new(RATE);
        // At 100 BPM a step lasts 48000 * 60 / 400 = 7200 frames.
        sequencer.apply(Command::SetTempo { bpm: 100.0 }, 0);
        let hits = [
            hit(0, 36, 100),
            hit(0, 42, 60),
            hit(2, 38, 127),
            // Beyond the 4 steps, and silent: both ignored.
            hit(4, 49, 100),
            hit(1, 46, 0),
        ];
        preview(&mut sequencer, &hits, 4, 0);
        let notes = timed_notes(&mut sequencer, 0, 12 * 7200 - BLOCK);
        let mut expected = Vec::new();
        for start in [0, 4, 8] {
            let at = |step: usize| block_of(((start + step) * 7200) as f64);
            expected.extend([(at(0), 36, 100), (at(0), 42, 60), (at(2), 38, 127)]);
        }
        assert_eq!(notes, expected);
    }

    #[test]
    fn a_preview_follows_swing_from_its_own_step_zero() {
        let mut sequencer = Sequencer::new(RATE);
        sequencer.apply(Command::SetSwing { amount: 0.25 }, 0);
        let hits = [hit(0, 36, 100), hit(1, 42, 100)];
        // Starts part way into a block, at frame 1000: step 1 is swung by 1500 frames.
        preview(&mut sequencer, &hits, 2, 1000);
        let notes = timed_notes(&mut sequencer, 960, 960 + 2 * 6000);
        assert_eq!(
            notes,
            [
                (960, 36, 100),
                (block_of(1000.0 + 6000.0 + 1500.0), 42, 100)
            ]
        );
    }

    #[test]
    fn a_preview_is_hidden_from_the_playhead_and_from_project_play() {
        let mut sequencer = playing(0);
        // Steps 0 to 2 fire; frame 17984 is the block boundary before step 3.
        let start = 17_984;
        assert_eq!(run(&mut sequencer, 0, start).len(), 3);
        assert!(sequencer.plays_project());
        assert_eq!(sequencer.history.at(start - 1), Some(pat(0, 2)));

        preview(&mut sequencer, &[hit(0, 36, 100)], 16, start);
        let fired = run(&mut sequencer, start, start + 5 * 6000 - BLOCK);
        assert_eq!(fired.len(), 5, "the preview steps fire");
        assert!(!sequencer.plays_project());
        assert!(sequencer.plays_preview());
        // From the moment the preview starts, the playhead shows nothing.
        assert_eq!(sequencer.history.at(start), None);
        assert_eq!(sequencer.history.at(start + 4 * 6000), None);
        // A cue does nothing while the preview plays.
        sequencer.apply(Command::CuePattern { pattern: 1 }, 8 * 6000);
        assert_eq!(sequencer.cue, None);
    }

    #[test]
    fn stop_preview_stops_only_the_preview() {
        let mut sequencer = Sequencer::new(RATE);
        preview(&mut sequencer, &[hit(0, 36, 100)], 1, 0);
        assert_eq!(timed_notes(&mut sequencer, 0, 3 * 6000 - BLOCK).len(), 3);
        sequencer.apply(Command::StopPreview, 3 * 6000 - BLOCK);
        assert!(!sequencer.plays_preview());
        assert!(timed_notes(&mut sequencer, 3 * 6000 - BLOCK, 10 * 6000).is_empty());

        // While a project pattern plays, stop_preview leaves it playing.
        sequencer.apply(
            Command::PlayPattern {
                pattern: 0,
                from: 0,
            },
            10 * 6000,
        );
        sequencer.apply(Command::StopPreview, 10 * 6000);
        assert!(sequencer.plays_project());

        // Stop stops the preview too.
        preview(&mut sequencer, &[hit(0, 36, 100)], 1, 11 * 6000);
        sequencer.apply(Command::Stop, 11 * 6000);
        assert!(!sequencer.clock.playing);
    }

    #[test]
    fn project_play_stops_the_preview_and_the_preview_stops_project_play() {
        let mut sequencer = Sequencer::new(RATE);
        sequencer.apply(Command::SetRowKey { row: 0, key: 38 }, 0);
        sequencer.apply(Command::step(0, 0, 0, 90).unwrap(), 0);
        preview(&mut sequencer, &[hit(0, 36, 100)], 1, 0);
        assert_eq!(timed_notes(&mut sequencer, 0, 6000 - BLOCK), [(0, 36, 100)]);

        sequencer.apply(
            Command::PlayPattern {
                pattern: 0,
                from: 0,
            },
            6000 - BLOCK,
        );
        assert!(!sequencer.plays_preview());
        let notes = timed_notes(&mut sequencer, 6000 - BLOCK, 17 * 6000);
        assert!(notes.iter().all(|&(_, key, _)| key == 38), "{notes:?}");
        assert_eq!(notes.len(), 2);

        // Starting a preview stops the pattern at once.
        preview(&mut sequencer, &[hit(0, 36, 100)], 1, 17 * 6000);
        assert!(!sequencer.plays_project());
        let notes = timed_notes(&mut sequencer, 17 * 6000, 33 * 6000);
        assert!(notes.iter().all(|&(_, key, _)| key == 36), "{notes:?}");

        // Song play stops it too.
        set_song(&mut sequencer, &[0], 33 * 6000);
        sequencer.apply(Command::PlaySong { from: 0 }, 33 * 6000);
        assert!(!sequencer.plays_preview());
        assert!(sequencer.plays_project());
    }

    #[test]
    fn a_preview_leaves_patterns_rows_and_song_unchanged() {
        let mut sequencer = Sequencer::new(RATE);
        sequencer.apply(Command::SetRowKey { row: 0, key: 36 }, 0);
        // A muted row on the same key does not mute the preview.
        sequencer.apply(Command::SetRowGain { row: 0, gain: 0.0 }, 0);
        sequencer.apply(Command::step(2, 0, 5, 100).unwrap(), 0);
        sequencer.apply(Command::length(2, 32).unwrap(), 0);
        set_song(&mut sequencer, &[2, 0], 0);
        let keys = sequencer.keys;
        let gains = sequencer.gains;
        let patterns = sequencer.patterns.clone();
        let lengths = sequencer.lengths;
        let song = (sequencer.song, sequencer.song_len);

        let hits: Vec<_> = (0..MAX_STEPS).map(|step| hit(step, 36, 77)).collect();
        preview(&mut sequencer, &hits, 1000, 0);
        assert_eq!(sequencer.preview_len, MAX_STEPS, "steps are clamped");
        let notes = timed_notes(&mut sequencer, 0, 4 * 6000 - BLOCK);
        assert_eq!(notes.len(), 4);
        assert!(
            notes
                .iter()
                .all(|&(_, key, velocity)| (key, velocity) == (36, 77))
        );
        preview(&mut sequencer, &hits, 0, 4 * 6000);
        assert_eq!(sequencer.preview_len, 1, "steps are clamped");

        assert_eq!(sequencer.keys, keys);
        assert_eq!(sequencer.gains, gains);
        assert!(sequencer.patterns == patterns);
        assert_eq!(sequencer.lengths, lengths);
        assert_eq!((sequencer.song, sequencer.song_len), song);
    }

    #[test]
    fn a_new_preview_replaces_the_old_one() {
        let mut sequencer = Sequencer::new(RATE);
        preview(&mut sequencer, &[hit(0, 36, 100), hit(1, 38, 100)], 2, 0);
        // 6016 is a block boundary just after step 1 of the first preview is due.
        timed_notes(&mut sequencer, 0, 6016);
        preview(&mut sequencer, &[hit(0, 42, 50), hit(0, 42, 70)], 2, 6016);
        let notes = timed_notes(&mut sequencer, 6016, 6016 + 4 * 6000 - BLOCK);
        // It restarts at step 0 now; the old hits are gone; the louder of two equal hits counts.
        assert_eq!(
            notes,
            [(6016, 42, 70), (block_of(6016.0 + 2.0 * 6000.0), 42, 70)]
        );
    }

    #[test]
    fn a_preview_larger_than_the_queue_arrives_whole() {
        let mut sequencer = Sequencer::new(RATE);
        let (mut commands, mut consumer) = command_queue();
        let hits: Vec<_> = (0..MAX_STEPS)
            .flat_map(|step| (0..20).map(move |k| hit(step, 30 + k, 100)))
            .collect();
        for command in Command::preview(&hits, MAX_STEPS) {
            commands.send(command);
        }
        assert!(!commands.backlog.is_empty());
        drain(&mut consumer, &mut sequencer, 0);
        assert!(
            !sequencer.plays_preview(),
            "not started until the last hit is in"
        );
        while !commands.flush() {
            drain(&mut consumer, &mut sequencer, 0);
        }
        drain(&mut consumer, &mut sequencer, 0);
        assert!(sequencer.plays_preview());
        let set = sequencer
            .preview
            .iter()
            .flatten()
            .filter(|&&v| v > 0)
            .count();
        assert_eq!(set, MAX_STEPS * 20);
    }

    #[test]
    fn preview_commands_count_as_transport() {
        let commands: Vec<_> = Command::preview(&[hit(0, 36, 200), hit(0, 200, 1)], 4).collect();
        assert_eq!(
            commands,
            [
                Command::ClearPreview,
                Command::SetPreviewHit {
                    step: 0,
                    key: 36,
                    velocity: 127
                },
                Command::PlayPreview { steps: 4 },
            ]
        );
        let transport: Vec<_> = commands.iter().map(Command::is_transport).collect();
        assert_eq!(transport, [false, false, true]);
        assert!(Command::StopPreview.is_transport());
    }

    #[test]
    fn a_preview_plays_in_the_renderer_and_goes_to_midi_out() {
        let font = soundfont_or_skip!();
        let (mut commands, mut renderer, mut events) = midi_renderer(&font, 38);
        // Row 0 plays key 38 in pattern 0; the preview must not.
        commands.send(Command::step(0, 0, 0, 100).unwrap());
        for command in Command::preview(&[hit(0, 36, 110), hit(2, 42, 90)], 4) {
            commands.send(command);
        }
        let mut out = vec![0.0_f32; 2 * 5900];
        renderer.render(&mut out, 2);
        assert!(energy(&out) > 1e-3, "the preview kick sounds");
        assert_eq!(published(&renderer), None);
        assert!(!renderer.status.playing.load(Ordering::Relaxed));
        assert!(renderer.status.previewing.load(Ordering::Relaxed));
        assert_eq!(renderer.status.transport.load(Ordering::Relaxed), 1);
        assert_eq!(renderer.note_off_at[36], renderer.ring_frames);

        commands.send(Command::SetMidiOut { on: true });
        let mut out = vec![0.0_f32; 2 * 8 * 6000];
        renderer.render(&mut out, 2);
        let sent = midi_sent(&mut events);
        assert_note_offs_between(&sent);
        let sent = without_note_offs(&sent);
        assert_eq!(
            sent,
            [
                vec![0xC9, 0],
                vec![0xB9, 7, 127],
                vec![0x99, 42, 90],
                vec![0x99, 36, 110],
                vec![0x99, 42, 90],
                vec![0x99, 36, 110],
            ]
        );

        commands.send(Command::StopPreview);
        renderer.render(&mut out, 2);
        assert!(!renderer.status.previewing.load(Ordering::Relaxed));
        assert_eq!(renderer.status.transport.load(Ordering::Relaxed), 2);
        let sent = messages(&midi_sent(&mut events));
        assert!(sent.iter().all(|m| m[0] != 0x99), "{sent:?}");
    }

    /// Asserts that `position` is in pattern mode at about `step` of `pattern`.
    fn assert_near(position: Option<HeardPosition>, pattern: usize, step: f64) {
        let Some(position) = position else {
            panic!("no position, expected step {step}");
        };
        assert_eq!(position.pattern, pattern);
        assert_eq!(position.song_index, None);
        assert!(
            (position.step - step).abs() < 0.02,
            "step {} instead of {step}",
            position.step
        );
    }

    #[test]
    fn the_heard_position_is_in_straight_time() {
        for swing in [0.0, 0.25, 0.5] {
            let mut sequencer = playing(0);
            sequencer.apply(Command::swing(swing).unwrap(), 0);
            // Step 4 has fired; step 5, on the grid at 30 000, has not.
            let now = 468 * BLOCK;
            assert_eq!(run(&mut sequencer, 0, now).last().unwrap().1, pat(0, 4));
            for step in [0.0, 0.5, 1.0, 2.9, 3.0, 4.25] {
                assert_near(sequencer.heard_position(step * 6000.0), 0, step);
            }
            // The next step's grid time has passed but it has not fired: the position goes on
            // from it, also for a hit a little in the future.
            for step in [5.0, 5.1, 5.9, 6.5] {
                assert_near(sequencer.heard_position(step * 6000.0), 0, step);
            }
            assert_eq!(sequencer.heard_position(-1.0), None, "before play started");
        }
    }

    #[test]
    fn the_heard_position_wraps_at_the_pattern_end_and_knows_the_song() {
        let mut sequencer = playing(0);
        sequencer.apply(
            Command::SetLength {
                pattern: 0,
                steps: 4,
            },
            0,
        );
        // Step 3 has fired; step 0 comes next at 24 000.
        let now = 3 * 6000 + 10 * BLOCK;
        run(&mut sequencer, 0, now);
        assert_near(sequencer.heard_position(23_400.0), 0, 3.9);
        assert_near(sequencer.heard_position(24_600.0), 0, 0.1);

        let mut sequencer = Sequencer::new(RATE);
        set_song(&mut sequencer, &[2, 3], 0);
        sequencer.apply(Command::PlaySong { from: 0 }, 0);
        run(&mut sequencer, 0, 17 * 6000);
        let position = sequencer.heard_position(16.5 * 6000.0).unwrap();
        assert_eq!(position.pattern, 3);
        assert_eq!(position.song_index, Some(1));
        assert!((position.step - 0.5).abs() < 1e-9);
    }

    #[test]
    fn there_is_no_heard_position_while_stopped_or_previewing() {
        let mut sequencer = playing(0);
        run(&mut sequencer, 0, 20_032);
        sequencer.apply(Command::Stop, 20_032);
        assert_near(sequencer.heard_position(19_000.0), 0, 19_000.0 / 6000.0);
        assert_eq!(sequencer.heard_position(20_032.0), None);
        assert_eq!(sequencer.heard_position(50_000.0), None);

        preview(&mut sequencer, &[hit(0, 36, 100)], 4, 30_016);
        run(&mut sequencer, 30_016, 40_000);
        assert_eq!(sequencer.heard_position(35_000.0), None);
        assert_eq!(sequencer.heard_position(41_000.0), None);
    }

    #[test]
    fn the_heard_position_is_none_before_the_history() {
        let mut sequencer = playing(0);
        sequencer.apply(
            Command::SetLength {
                pattern: 0,
                steps: 64,
            },
            0,
        );
        run(&mut sequencer, 0, 40 * 6000);
        assert_eq!(sequencer.heard_position(100.0), None);
        assert_near(sequencer.heard_position(39.5 * 6000.0), 0, 39.5);
    }

    #[test]
    fn a_live_hit_takes_its_rows_gain() {
        let mut sequencer = Sequencer::new(RATE);
        sequencer.apply(Command::SetRowKey { row: 0, key: 36 }, 0);
        sequencer.apply(Command::row_gain(0, 0.5).unwrap(), 0);
        sequencer.apply(Command::SetRowKey { row: 1, key: 38 }, 0);
        sequencer.apply(Command::row_gain(1, 0.0).unwrap(), 0);
        // A second row on the same key does not count; the first does.
        sequencer.apply(Command::SetRowKey { row: 2, key: 36 }, 0);
        assert_eq!(sequencer.live_velocity(36, 100), 50);
        assert_eq!(sequencer.live_velocity(38, 100), 0);
        assert_eq!(sequencer.live_velocity(42, 100), 100);
    }

    /// A renderer with the MIDI, live hit and heard hit queues taken out.
    fn live_renderer(
        font: &Arc<SoundFont>,
    ) -> (
        Commands,
        Renderer,
        MidiEvents,
        LiveHits,
        rtrb::Consumer<HeardHit>,
    ) {
        let (commands, mut renderer) = Renderer::new(font, RATE).unwrap();
        let events = renderer.take_midi_events().unwrap();
        let live = renderer.take_live_hits().unwrap();
        assert!(renderer.take_live_hits().is_none());
        let heard = renderer.take_heard_hits().unwrap();
        assert!(renderer.take_heard_hits().is_none());
        (commands, renderer, events, live, heard)
    }

    /// Queues a live hit made while the player heard `frame`, for the next `render` call, which
    /// must follow at once.
    fn push_live(renderer: &Renderer, live: &mut LiveHits, frame: f64, key: u8, sound: bool) {
        let block = renderer.left.len();
        let call_start = renderer.clock - (block - renderer.read) as u64;
        let heard_now = call_start as f64 - renderer.latency_frames as f64;
        let seconds = (frame - heard_now) / f64::from(RATE);
        let now = Instant::now();
        let at = if seconds < 0.0 {
            now - Duration::from_secs_f64(-seconds)
        } else {
            now + Duration::from_secs_f64(seconds)
        };
        live.push(LiveHit {
            at,
            key,
            velocity: 100,
            sound,
        })
        .unwrap();
    }

    /// Every heard hit reported so far, oldest first.
    fn heard_hits(heard: &mut rtrb::Consumer<HeardHit>) -> Vec<HeardHit> {
        std::iter::from_fn(|| heard.pop().ok()).collect()
    }

    #[test]
    fn a_live_hit_plays_and_is_reported() {
        let font = soundfont_or_skip!();
        let (_commands, mut renderer, _events, mut live, mut heard) = live_renderer(&font);
        push_live(&renderer, &mut live, 0.0, 38, true);
        let mut out = vec![0.0_f32; 2 * 4_800];
        renderer.render(&mut out, 2);
        assert!(energy(&out) > 1e-3);
        assert_ne!(renderer.note_off_at[38], NO_NOTE_OFF);
        assert_eq!(
            heard_hits(&mut heard),
            [HeardHit {
                key: 38,
                velocity: 100,
                position: None,
            }]
        );
    }

    #[test]
    fn a_live_hit_goes_to_midi_out_at_its_rows_gain() {
        let font = soundfont_or_skip!();
        let (mut commands, mut renderer, mut events, mut live, mut heard) = live_renderer(&font);
        commands.send(Command::SetRowKey { row: 0, key: 36 });
        commands.send(Command::row_gain(0, 0.5).unwrap());
        commands.send(Command::SetMidiOut { on: true });
        push_live(&renderer, &mut live, 0.0, 36, true);
        push_live(&renderer, &mut live, 0.0, 42, true);
        let mut out = vec![1.0_f32; 2 * 640];
        renderer.render(&mut out, 2);
        assert!(out.iter().all(|&s| s == 0.0));
        assert_eq!(
            without_note_offs(&midi_sent(&mut events)),
            [
                vec![0xC9, 0],
                vec![0xB9, 7, 127],
                vec![0x99, 36, 50],
                vec![0x99, 42, 100]
            ]
        );
        // The report keeps the velocity the player hit with.
        let velocities: Vec<_> = heard_hits(&mut heard).iter().map(|h| h.velocity).collect();
        assert_eq!(velocities, [100, 100]);
    }

    #[test]
    fn a_muted_or_unsounded_live_hit_is_silent_but_reported() {
        let font = soundfont_or_skip!();
        for midi_out in [false, true] {
            let (mut commands, mut renderer, mut events, mut live, mut heard) =
                live_renderer(&font);
            commands.send(Command::SetRowKey { row: 0, key: 36 });
            commands.send(Command::row_gain(0, 0.0).unwrap());
            commands.send(Command::SetMidiOut { on: midi_out });
            push_live(&renderer, &mut live, 0.0, 36, true);
            push_live(&renderer, &mut live, 0.0, 38, false);
            let mut out = vec![1.0_f32; 2 * 4_800];
            renderer.render(&mut out, 2);
            assert!(out.iter().all(|&s| s == 0.0), "MIDI out {midi_out}");
            assert_eq!(renderer.note_off_at[36], NO_NOTE_OFF);
            assert_eq!(renderer.note_off_at[38], NO_NOTE_OFF);
            let sent = messages(&midi_sent(&mut events));
            assert!(sent.iter().all(|m| m[0] != MIDI_NOTE_ON), "{sent:?}");
            let keys: Vec<_> = heard_hits(&mut heard).iter().map(|h| h.key).collect();
            assert_eq!(keys, [36, 38]);
        }
    }

    #[test]
    fn a_live_hit_reports_the_step_the_player_heard() {
        let font = soundfont_or_skip!();
        for swing in [0.0, 0.25] {
            let (mut commands, mut renderer, _events, mut live, mut heard) = live_renderer(&font);
            renderer.set_latency(960);
            commands.send(Command::swing(swing).unwrap());
            commands.send(Command::PlayPattern {
                pattern: 0,
                from: 0,
            });
            // Into step 6 of what is rendered; the listener hears 960 frames less.
            let mut out = vec![0.0_f32; 2 * (6 * 6000 + 3000)];
            renderer.render(&mut out, 2);
            // Not at 0.0: the test's clock runs a little ahead of the renderer's, so that hit
            // could land just before play started.
            let steps = [0.1, 1.0, 2.5, 3.0, 4.9, 5.0];
            for step in steps {
                push_live(&renderer, &mut live, step * 6000.0, 36, true);
            }
            let mut out = vec![0.0_f32; 2 * 640];
            renderer.render(&mut out, 2);
            let reported = heard_hits(&mut heard);
            assert_eq!(reported.len(), steps.len());
            for (hit, step) in reported.iter().zip(steps) {
                assert_near(hit.position, 0, step);
            }

            // After a stop, a hit heard after it has no position.
            commands.send(Command::Stop);
            let mut out = vec![0.0_f32; 2 * 6000];
            renderer.render(&mut out, 2);
            let stopped = renderer.clock as f64 - 10.0;
            push_live(&renderer, &mut live, stopped - 960.0, 36, true);
            renderer.render(&mut out, 2);
            assert_eq!(heard_hits(&mut heard)[0].position, None);
        }
    }
}
