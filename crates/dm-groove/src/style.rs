//! Reads the drum parts of a Yamaha keyboard style: one groove per section.
//!
//! A Yamaha style starts as a Standard MIDI File with one track. Markers in the track start its
//! sections ("Main A", "Fill In AA" and so on). The setup bar before the first section sets the
//! tempo, the time signature and each channel's kit. Other blocks (CASM, OTS, MDB) follow the
//! track; they are cut off before the track is read.
//!
//! The drums are channel 10 (Rhythm) and channel 9 (Sub-Rhythm, the extra percussion). Both are
//! merged into one groove. Each note's key goes through its channel's kit in the key map
//! ([`yamaha_kits`]), which puts every sound the app has a row for on its General MIDI key and
//! leaves the rest out. `docs/DESIGN.md` ("Import grooves from a keyboard style") explains why.

use std::ops::RangeInclusive;

use midly::{MetaMessage, MidiMessage, Smf, Timing, TrackEventKind};

use crate::Refusal;
use crate::groove::{self, Fitted, MAX_MIDI_BYTES, Source};
use crate::midi::{MidiFile, Note};

#[path = "yamaha_kits.rs"]
mod yamaha_kits;

/// One drum kit of a Yamaha keyboard, and where each of its keys goes.
pub(crate) struct YamahaKit {
    /// The bank select MSB (controller 0) that picks the kit.
    pub msb: u8,
    /// The program change that picks the kit, counted from 0.
    pub program: u8,
    /// The kit's name, as the keyboard shows it.
    pub name: &'static str,
    /// The General MIDI program of the nearest kit in the app's short list. `None` for
    /// percussion and effect kits.
    pub similar: Option<u8>,
    /// `(the kit's key, the General MIDI key it becomes)`. A key not listed is left out.
    pub keys: &'static [(u8, u8)],
}

impl YamahaKit {
    /// The kit a bank select MSB and program pick, if the table knows it.
    pub(crate) fn find(msb: u8, program: u8) -> Option<&'static YamahaKit> {
        yamaha_kits::KITS
            .iter()
            .find(|kit| kit.msb == msb && kit.program == program)
    }

    /// The General MIDI key for one of the kit's keys, or `None` to leave the note out.
    fn key(&self, key: u8) -> Option<u8> {
        self.keys
            .iter()
            .find(|&&(own, _)| own == key)
            .map(|&(_, general_midi)| general_midi)
    }
}

/// The markers that start a section, spelled as Yamaha's keyboards write them.
const SECTION_MARKERS: [&str; 17] = [
    "Intro A",
    "Intro B",
    "Intro C",
    "Intro D",
    "Main A",
    "Main B",
    "Main C",
    "Main D",
    "Fill In AA",
    "Fill In BB",
    "Fill In CC",
    "Fill In DD",
    "Fill In BA",
    "Ending A",
    "Ending B",
    "Ending C",
    "Ending D",
];
/// Channel 10 (Rhythm) and channel 9 (Sub-Rhythm), counted from 0. They are listed in this
/// order wherever the code keeps something per channel.
const DRUM_CHANNELS: [u8; 2] = [9, 8];
/// The keys a kit the table does not know keeps: General MIDI's drums and percussion.
const GENERAL_MIDI_KEYS: RangeInclusive<u8> = 35..=81;

/// One section of a style: its marker and what it became.
#[derive(Debug)]
pub(crate) struct Section {
    /// The marker, such as `Main A`.
    pub marker: String,
    pub result: Result<StyleGroove, Refusal>,
}

/// An accepted section.
#[derive(Debug)]
pub(crate) struct StyleGroove {
    pub fitted: Fitted,
    /// The names of the kits whose notes are in the groove, channel 10's first, joined by
    /// ` + `. `None` when neither kit is in the table.
    pub kit: Option<String>,
    /// The `similar` of channel 10's kit, or of channel 9's when channel 10's has none.
    pub similar_kit: Option<u8>,
}

/// What the reader keeps from the style's track.
#[derive(Default)]
struct Style {
    ppq: u32,
    /// The tempo and time signature the setup bar sets last, if any.
    setup_tempo: Option<f64>,
    setup_time_signature: Option<(u8, u8)>,
    /// The kit of channel 10 and of channel 9, if the setup bar picks one the table knows.
    kits: [Option<&'static YamahaKit>; 2],
    /// `(tick, marker)` of every section marker.
    markers: Vec<(u64, String)>,
    /// Tempos and time signatures after the setup bar, as in [`MidiFile`].
    tempos: Vec<(u64, f64)>,
    time_signatures: Vec<(u64, u8, u8)>,
    /// Note-ons on the drum channels, with the keys the style plays.
    notes: Vec<Note>,
    end: u64,
}

/// Whether the bytes are a style: a Standard MIDI File whose first track has a section marker.
pub(crate) fn is_style(bytes: &[u8]) -> bool {
    read(bytes).is_some()
}

/// Reads a style's sections, in the order the track holds them. `None` if the bytes are not a
/// style.
pub(crate) fn sections(bytes: &[u8]) -> Option<Vec<Section>> {
    let style = read(bytes)?;
    let sections = style
        .markers
        .iter()
        .enumerate()
        .map(|(index, (start, marker))| {
            // A section runs to the next section marker, or to the end of the track.
            let end = style
                .markers
                .get(index + 1)
                .map_or(style.end, |next| next.0)
                .max(*start);
            Section {
                marker: marker.clone(),
                result: section(&style, *start, end),
            }
        })
        .collect();
    Some(sections)
}

/// The header and the first track, without what follows. `None` if the bytes do not start that
/// way.
fn midi_part(bytes: &[u8]) -> Option<&[u8]> {
    let chunk_length = |at: usize| -> Option<usize> {
        let length: [u8; 4] = bytes.get(at + 4..at + 8)?.try_into().ok()?;
        usize::try_from(u32::from_be_bytes(length)).ok()
    };
    if !bytes.starts_with(b"MThd") {
        return None;
    }
    let track = 8usize.checked_add(chunk_length(0)?)?;
    if bytes.get(track..track + 4)? != b"MTrk" {
        return None;
    }
    let end = track.checked_add(8)?.checked_add(chunk_length(track)?)?;
    bytes.get(..end)
}

fn read(bytes: &[u8]) -> Option<Style> {
    let part = midi_part(bytes)?;
    if part.len() > MAX_MIDI_BYTES {
        return None;
    }
    let smf = Smf::parse(part).ok()?;
    let ppq = match smf.header.timing {
        Timing::Metrical(t) => u32::from(t.as_int()),
        Timing::Timecode(..) => return None,
    };
    if ppq == 0 {
        return None;
    }
    let mut style = Style {
        ppq,
        ..Default::default()
    };
    // Per drum channel: the last bank select MSB, and the bank and program of the last program
    // change, in the setup bar.
    let mut msb = [0u8; 2];
    let mut programs: [Option<(u8, u8)>; 2] = [None; 2];
    let mut tick = 0u64;
    for event in smf.tracks.first()? {
        tick += u64::from(event.delta.as_int());
        let setup = style.markers.is_empty();
        match event.kind {
            TrackEventKind::Meta(MetaMessage::Marker(text)) => {
                let text = String::from_utf8_lossy(text);
                let text = text.trim();
                if SECTION_MARKERS.contains(&text) {
                    style.markers.push((tick, text.to_owned()));
                }
            }
            TrackEventKind::Meta(MetaMessage::Tempo(t)) if t.as_int() > 0 => {
                let bpm = 60_000_000.0 / f64::from(t.as_int());
                if setup {
                    style.setup_tempo = Some(bpm);
                } else {
                    style.tempos.push((tick, bpm));
                }
            }
            TrackEventKind::Meta(MetaMessage::TimeSignature(n, d, _, _)) => {
                let denominator = 1u8 << d.min(7);
                if setup {
                    style.setup_time_signature = Some((n, denominator));
                } else {
                    style.time_signatures.push((tick, n, denominator));
                }
            }
            TrackEventKind::Meta(MetaMessage::EndOfTrack) => style.end = style.end.max(tick),
            TrackEventKind::Midi { channel, message } => {
                let Some(slot) = DRUM_CHANNELS.iter().position(|&c| c == channel.as_int()) else {
                    continue;
                };
                match message {
                    MidiMessage::NoteOn { key, vel } if vel.as_int() > 0 => {
                        style.notes.push(Note {
                            tick,
                            key: key.as_int(),
                            velocity: vel.as_int(),
                            channel: channel.as_int(),
                        });
                    }
                    MidiMessage::Controller { controller, value }
                        if setup && controller.as_int() == 0 =>
                    {
                        msb[slot] = value.as_int();
                    }
                    MidiMessage::ProgramChange { program } if setup => {
                        programs[slot] = Some((msb[slot], program.as_int()));
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
    if style.markers.is_empty() {
        return None;
    }
    style.end = style.end.max(tick);
    style.kits = programs.map(|p| p.and_then(|(msb, program)| YamahaKit::find(msb, program)));
    Some(style)
}

/// Turns the notes between two ticks into a groove, or refuses them.
fn section(style: &Style, start: u64, end: u64) -> Result<StyleGroove, Refusal> {
    let inside = |tick: u64| tick >= start && tick < end;
    // Whether each drum channel has a note in the groove.
    let mut played = [false; 2];
    let mut notes = Vec::new();
    for note in style.notes.iter().filter(|n| inside(n.tick)) {
        let slot = usize::from(note.channel != DRUM_CHANNELS[0]);
        let key = match style.kits[slot] {
            Some(kit) => kit.key(note.key),
            None => GENERAL_MIDI_KEYS.contains(&note.key).then_some(note.key),
        };
        if let Some(key) = key {
            played[slot] = true;
            // One channel for both, so the groove rules see one drum part.
            notes.push(Note {
                tick: note.tick - start,
                key,
                velocity: note.velocity,
                channel: DRUM_CHANNELS[0],
            });
        }
    }

    let tempos = style
        .setup_tempo
        .map(|bpm| (0, bpm))
        .into_iter()
        .chain(
            style
                .tempos
                .iter()
                .filter(|t| inside(t.0))
                .map(|&(tick, bpm)| (tick - start, bpm)),
        )
        .collect();
    let time_signatures = style
        .setup_time_signature
        .map(|(n, d)| (0, n, d))
        .into_iter()
        .chain(
            style
                .time_signatures
                .iter()
                .filter(|t| inside(t.0))
                .map(|&(tick, n, d)| (tick - start, n, d)),
        )
        .collect();
    let file = MidiFile {
        ppq: style.ppq,
        tempos,
        time_signatures,
        notes,
        end_tick: end - start,
    };
    let fitted = groove::fit(&file, Source::Style)?;

    let kits: Vec<&YamahaKit> = style
        .kits
        .iter()
        .zip(played)
        .filter_map(|(kit, played)| kit.filter(|_| played))
        .collect();
    let kit = (!kits.is_empty()).then(|| {
        kits.iter()
            .map(|kit| kit.name)
            .collect::<Vec<_>>()
            .join(" + ")
    });
    Ok(StyleGroove {
        fitted,
        kit,
        similar_kit: kits.iter().find_map(|kit| kit.similar),
    })
}
