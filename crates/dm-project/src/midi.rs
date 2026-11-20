//! MIDI export: the song as a Standard MIDI File.
//!
//! The file is format 0: one track holding everything. It starts with the track name, the tempo,
//! the 4/4 time signature and a Program Change that picks the kit, then plays the song once.
//!
//! All notes are on MIDI channel 10, the General MIDI drum channel. General MIDI and Roland GS
//! players choose a drum kit with a Program Change on that channel alone; GS ignores Bank Select
//! for drum parts, and the bank number other systems expect differs (120 for GM2, 127 for XG).
//! So no Bank Select is sent, and the kit number is the project's program number unchanged.

use std::ops::Range;

use crate::{DEFAULT_TEMPO, MAX_SWING, Pattern, Project};

/// The extension of an exported MIDI file, without the dot.
pub const MIDI_EXTENSION: &str = "mid";
/// The file's time resolution: ticks in one quarter note, which is one beat.
pub const TICKS_PER_QUARTER: u16 = 96;
/// Ticks in one step. A beat holds four steps.
pub const TICKS_PER_STEP: u32 = TICKS_PER_QUARTER as u32 / 4;
/// How long each note is held: half a step. Drum sounds ring out on their own, so the length
/// mostly matters to software that draws the notes.
const NOTE_TICKS: u32 = TICKS_PER_STEP / 2;
/// The velocity sent with every note off.
const NOTE_OFF_VELOCITY: u8 = 64;
/// MIDI channel 10, counted from 0 as the status byte does.
const DRUM_CHANNEL: u8 = 9;
const NOTE_OFF: u8 = 0x80 | DRUM_CHANNEL;
const NOTE_ON: u8 = 0x90 | DRUM_CHANNEL;
const PROGRAM_CHANGE: u8 = 0xC0 | DRUM_CHANNEL;
/// The name written in the track.
const TRACK_NAME: &str = "Drums";
/// The largest number a tempo meta event holds: three bytes of microseconds per beat.
const MAX_TEMPO_MICROSECONDS: f64 = 0xFF_FFFF as f64;

/// Events at the same tick are written in this order, so a note that ends where the next one on
/// the same key starts is turned off before it is turned on again.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Order {
    Setup,
    NoteOff,
    NoteOn,
}

/// Writes the song as a Standard MIDI File.
///
/// The song plays once, in order, each entry for its pattern's playing length; hidden bars are
/// left out. An empty song plays the first pattern once. Rows that mute or solo silence are left
/// out, and every note's velocity is scaled by its row's volume. With swing, every odd step starts
/// `swing * 24` ticks late, rounded. The track ends at the end of the song, or after the last note
/// off if that is later.
pub fn to_midi(project: &Project) -> Vec<u8> {
    let mut events: Vec<(u32, Order, Vec<u8>)> = Vec::new();
    let mut setup = |bytes: Vec<u8>| events.push((0, Order::Setup, bytes));
    setup(meta(0x03, TRACK_NAME.as_bytes()));
    setup(meta(0x51, &tempo_bytes(project.tempo)));
    // 4/4, a click every quarter note (24 MIDI clocks), eight 32nd notes per quarter note.
    setup(meta(0x58, &[4, 2, 24, 8]));
    setup(vec![PROGRAM_CHANGE, project.kit.min(127)]);

    let mut start = 0;
    for pattern in play_order(project) {
        for note in pattern_notes(project, pattern, 0..pattern.length(), TICKS_PER_STEP) {
            let tick = start + note.tick;
            events.push((tick, Order::NoteOn, vec![NOTE_ON, note.key, note.velocity]));
            events.push((
                tick + NOTE_TICKS,
                Order::NoteOff,
                vec![NOTE_OFF, note.key, NOTE_OFF_VELOCITY],
            ));
        }
        start += pattern.length() as u32 * TICKS_PER_STEP;
    }
    // A stable sort keeps rows in order among events of the same tick and kind.
    events.sort_by_key(|&(tick, order, _)| (tick, order));
    let last = events.last().map_or(0, |&(tick, _, _)| tick);
    smf(TICKS_PER_QUARTER, &events, start.max(last))
}

/// One note to write: its start in ticks, counted from the first step asked for, its key and its
/// velocity.
pub(crate) struct Note {
    pub tick: u32,
    pub key: u8,
    pub velocity: u8,
}

/// The notes a pattern plays in `steps`, with `ticks_per_step` ticks in each step. They come in
/// step order, and in row order within a step.
///
/// Rows that mute or solo silence are left out, and every velocity is scaled by its row's
/// volume. With swing, every odd step starts late by the swing times one step, rounded. Odd
/// means odd in the pattern, so `steps` should start on an even step.
pub(crate) fn pattern_notes(
    project: &Project,
    pattern: &Pattern,
    steps: Range<usize>,
    ticks_per_step: u32,
) -> Vec<Note> {
    let gains: Vec<u32> = (0..project.rows.len())
        .map(|row| u32::from(project.gain(row)))
        .collect();
    let swing = (project.swing.clamp(0.0, MAX_SWING) * ticks_per_step as f32).round() as u32;
    let first = steps.start;
    let mut notes = Vec::new();
    for step in steps {
        let tick = (step - first) as u32 * ticks_per_step + if step % 2 == 1 { swing } else { 0 };
        for (row, info) in project.rows.iter().enumerate() {
            let velocity = u32::from(pattern.velocity(row, step));
            if velocity == 0 || gains[row] == 0 {
                continue;
            }
            notes.push(Note {
                tick,
                key: info.key.min(127),
                velocity: ((velocity * gains[row] + 50) / 100).clamp(1, 127) as u8,
            });
        }
    }
    notes
}

/// A format-0 Standard MIDI File with one track. `events` holds each event's tick and bytes,
/// sorted by tick; the middle field is only there for sorting and is not written. The track
/// ends at `end`, which must not be before the last event.
pub(crate) fn smf<O>(division: u16, events: &[(u32, O, Vec<u8>)], end: u32) -> Vec<u8> {
    let mut track = Vec::new();
    let mut previous = 0;
    for (tick, _, bytes) in events {
        write_vlq(&mut track, tick - previous);
        track.extend_from_slice(bytes);
        previous = *tick;
    }
    write_vlq(&mut track, end - previous);
    track.extend_from_slice(&meta(0x2F, &[]));

    let mut file = Vec::with_capacity(22 + track.len());
    file.extend_from_slice(b"MThd");
    file.extend_from_slice(&6u32.to_be_bytes());
    file.extend_from_slice(&0u16.to_be_bytes()); // format 0
    file.extend_from_slice(&1u16.to_be_bytes()); // one track
    file.extend_from_slice(&division.to_be_bytes());
    file.extend_from_slice(b"MTrk");
    let length = u32::try_from(track.len()).expect("a song is far smaller than 4 GiB");
    file.extend_from_slice(&length.to_be_bytes());
    file.extend_from_slice(&track);
    file
}

/// The patterns the song plays, in order. An empty song plays the first pattern.
fn play_order(project: &Project) -> Vec<&Pattern> {
    if project.song.is_empty() {
        return project.patterns.first().into_iter().collect();
    }
    project
        .song
        .iter()
        .filter_map(|&slot| project.patterns.iter().find(|p| p.slot == slot))
        .collect()
}

/// The tempo as microseconds per beat, in three bytes. A tempo that is not a positive number
/// counts as the default.
pub(crate) fn tempo_bytes(tempo: f32) -> [u8; 3] {
    let bpm = if tempo.is_finite() && tempo > 0.0 {
        f64::from(tempo)
    } else {
        f64::from(DEFAULT_TEMPO)
    };
    let microseconds = (60_000_000.0 / bpm)
        .round()
        .clamp(1.0, MAX_TEMPO_MICROSECONDS) as u32;
    let [_, a, b, c] = microseconds.to_be_bytes();
    [a, b, c]
}

/// A meta event without its delta time.
pub(crate) fn meta(kind: u8, data: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0xFF, kind];
    write_vlq(&mut bytes, data.len() as u32);
    bytes.extend_from_slice(data);
    bytes
}

/// Writes a variable-length quantity: seven bits per byte, most significant first, with the top
/// bit set on every byte but the last.
pub(crate) fn write_vlq(out: &mut Vec<u8>, value: u32) {
    let mut groups = [0u8; 5];
    let mut count = 0;
    let mut rest = value;
    loop {
        groups[count] = (rest & 0x7F) as u8;
        count += 1;
        rest >>= 7;
        if rest == 0 {
            break;
        }
    }
    // The groups were collected least significant first; the last one written has no top bit.
    for (i, group) in groups[..count].iter().enumerate().rev() {
        out.push(if i > 0 { group | 0x80 } else { *group });
    }
}

#[cfg(test)]
#[path = "midi_tests.rs"]
mod tests;
