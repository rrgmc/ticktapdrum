//! Reads a Standard MIDI File into one flat list of notes, with tempo and time signatures.

use midly::{MetaMessage, MidiMessage, Smf, Timing, TrackEventKind};

#[derive(Debug, Clone, Copy)]
pub struct Note {
    pub tick: u64,
    pub key: u8,
    pub velocity: u8,
    /// Counted from 0, so 9 is MIDI channel 10.
    pub channel: u8,
}

#[derive(Debug, Default)]
pub struct MidiFile {
    pub format: u16,
    pub ppq: u32,
    /// `(tick, beats per minute)`.
    pub tempos: Vec<(u64, f64)>,
    /// `(tick, numerator, denominator)`.
    pub time_signatures: Vec<(u64, u8, u8)>,
    pub notes: Vec<Note>,
    /// The latest End of Track tick over all tracks.
    pub end_tick: u64,
}

pub fn parse(bytes: &[u8]) -> Result<MidiFile, String> {
    let smf = Smf::parse(bytes).map_err(|e| format!("not a MIDI file: {e}"))?;
    let ppq = match smf.header.timing {
        Timing::Metrical(t) => u32::from(t.as_int()),
        Timing::Timecode(..) => return Err("SMPTE timing is not supported".into()),
    };
    let mut file = MidiFile {
        format: match smf.header.format {
            midly::Format::SingleTrack => 0,
            midly::Format::Parallel => 1,
            midly::Format::Sequential => 2,
        },
        ppq,
        ..Default::default()
    };
    for track in &smf.tracks {
        let mut tick = 0u64;
        for event in track {
            tick += u64::from(event.delta.as_int());
            match event.kind {
                TrackEventKind::Midi { channel, message } => {
                    if let MidiMessage::NoteOn { key, vel } = message
                        && vel.as_int() > 0
                    {
                        file.notes.push(Note {
                            tick,
                            key: key.as_int(),
                            velocity: vel.as_int(),
                            channel: channel.as_int(),
                        });
                    }
                }
                TrackEventKind::Meta(MetaMessage::Tempo(t)) => {
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
    file.tempos.sort_by_key(|a| a.0);
    file.time_signatures.sort_by_key(|t| t.0);
    Ok(file)
}
