//! Reads a Standard MIDI File into one flat list of notes, with its tempos and time signatures.

use midly::{MetaMessage, MidiMessage, Smf, Timing, TrackEventKind};

/// One note-on with a velocity above 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Note {
    pub tick: u64,
    pub key: u8,
    pub velocity: u8,
    /// Counted from 0, so 9 is MIDI channel 10.
    pub channel: u8,
}

/// What the analysis needs from a MIDI file. Every list is sorted by tick.
#[derive(Debug, Default)]
pub struct MidiFile {
    /// Ticks in one quarter note. Never 0.
    pub ppq: u32,
    /// `(tick, beats per minute)`.
    pub tempos: Vec<(u64, f64)>,
    /// `(tick, numerator, denominator)`.
    pub time_signatures: Vec<(u64, u8, u8)>,
    pub notes: Vec<Note>,
    /// The latest End of Track tick over all tracks.
    pub end_tick: u64,
}

/// Reads the file, or `None` if it is not a Standard MIDI File, uses SMPTE timing, or claims 0
/// ticks per quarter note.
pub fn parse(bytes: &[u8]) -> Option<MidiFile> {
    let smf = Smf::parse(bytes).ok()?;
    let ppq = match smf.header.timing {
        Timing::Metrical(t) => u32::from(t.as_int()),
        Timing::Timecode(..) => return None,
    };
    if ppq == 0 {
        return None;
    }
    let mut file = MidiFile {
        ppq,
        ..Default::default()
    };
    for track in &smf.tracks {
        let mut tick = 0u64;
        for event in track {
            tick += u64::from(event.delta.as_int());
            match event.kind {
                TrackEventKind::Midi {
                    channel,
                    message: MidiMessage::NoteOn { key, vel },
                } if vel.as_int() > 0 => {
                    file.notes.push(Note {
                        tick,
                        key: key.as_int(),
                        velocity: vel.as_int(),
                        channel: channel.as_int(),
                    });
                }
                TrackEventKind::Meta(MetaMessage::Tempo(t)) if t.as_int() > 0 => {
                    file.tempos
                        .push((tick, 60_000_000.0 / f64::from(t.as_int())));
                }
                TrackEventKind::Meta(MetaMessage::TimeSignature(n, d, _, _)) => {
                    file.time_signatures.push((tick, n, 1u8 << d.min(7)));
                }
                TrackEventKind::Meta(MetaMessage::EndOfTrack) => {
                    file.end_tick = file.end_tick.max(tick);
                }
                _ => {}
            }
        }
    }
    file.notes.sort_by_key(|n| n.tick);
    file.tempos.sort_by_key(|t| t.0);
    file.time_signatures.sort_by_key(|t| t.0);
    Some(file)
}
