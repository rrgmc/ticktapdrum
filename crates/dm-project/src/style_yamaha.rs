//! The Yamaha writer: the parts of a style as a drum-only Yamaha style file (`.sty`).
//!
//! A Yamaha style is a format-0 Standard MIDI File with markers for its sections. A style that
//! plays only drums needs none of the extra blocks (CASM, OTS, MDB) that may follow the track.
//! The layout here is the one a PSR-SX720 loaded and played; `examples/yamaha_style.rs` wrote the
//! file that was tried, and `docs/research/arranger-style-export.md` has the details.
//!
//! Bar 1 is the setup: the time signature, the tempo, the `SFF1` marker, the style's name, the
//! `SInt` marker, the General MIDI reset and the drum kit. The sections follow from bar 2, each
//! starting on a bar line with its marker. Notes are on channel 10, the Rhythm part, at 1920
//! ticks per beat, with the same timing as the MIDI export.

use super::{Part, Section};
use crate::midi::{meta, pattern_notes, smf, tempo_bytes};
use crate::{Project, STEPS_PER_BAR};

/// The extension of a Yamaha style file, without the dot.
pub(super) const EXTENSION: &str = "sty";
/// The file's time resolution: ticks in one beat.
const TICKS_PER_BEAT: u16 = 1920;
/// Ticks in one step. A beat holds four steps.
pub(super) const TICKS_PER_STEP: u32 = TICKS_PER_BEAT as u32 / 4;
pub(super) const TICKS_PER_BAR: u32 = TICKS_PER_STEP * STEPS_PER_BAR as u32;
/// How long each note is held: half a step, as in the MIDI export.
const NOTE_TICKS: u32 = TICKS_PER_STEP / 2;
/// Channel 10 (Rhythm) and channel 9 (Sub-Rhythm), counted from 0 as the status byte does. The
/// notes go on Rhythm. Sub-Rhythm gets the same setup and no notes, as in the file the SX720
/// played.
const RHYTHM: u8 = 9;
const SUB_RHYTHM: u8 = 8;
/// The name used when the project's name has nothing printable in it.
const DEFAULT_NAME: &str = "TickTapDrum";

/// Yamaha's style has every section the app offers.
pub(super) fn has(_section: Section) -> bool {
    true
}

/// Yamaha's fills and its Break are exactly one bar.
pub(super) fn one_bar(section: Section) -> bool {
    matches!(section, Section::Fill(_) | Section::Break)
}

/// The marker that starts a section, spelled exactly as Yamaha's keyboards expect.
fn marker(section: Section) -> &'static str {
    const MAINS: [&str; 4] = ["Main A", "Main B", "Main C", "Main D"];
    const FILLS: [&str; 4] = ["Fill In AA", "Fill In BB", "Fill In CC", "Fill In DD"];
    match section {
        Section::Intro => "Intro A",
        Section::Main(i) => MAINS[i],
        Section::Fill(i) => FILLS[i],
        Section::Break => "Fill In BA",
        Section::Ending => "Ending A",
    }
}

/// Events at the same tick are written in this order. A section's last note offs come before
/// the next section's marker, and every note off before a note on, so a note that ends where the
/// next one on the same key starts is turned off first.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Order {
    Setup,
    NoteOff,
    Marker,
    NoteOn,
}

/// Writes the parts as a drum-only Yamaha style, in the order given, with `name` as the style's
/// name. A note that would ring past the end of its section is cut off there.
pub(super) fn write(project: &Project, parts: &[Part<'_>], name: &str) -> Vec<u8> {
    let mut events: Vec<(u32, Order, Vec<u8>)> = Vec::new();
    let mut setup = |bytes: Vec<u8>| events.push((0, Order::Setup, bytes));
    // 4/4, a click every quarter note (24 MIDI clocks), eight 32nd notes per quarter note.
    setup(meta(0x58, &[4, 2, 24, 8]));
    setup(meta(0x51, &tempo_bytes(project.tempo)));
    setup(meta(0x06, b"SFF1"));
    setup(meta(0x03, style_name(name).as_bytes()));
    setup(meta(0x06, b"SInt"));
    // General MIDI on, as a system exclusive message: F0, the length, then the data.
    setup(vec![0xF0, 5, 0x7E, 0x7F, 0x09, 0x01, 0xF7]);
    for channel in [SUB_RHYTHM, RHYTHM] {
        setup(vec![0xB0 | channel, 0, 127]); // bank select MSB: drum kits
        setup(vec![0xB0 | channel, 32, 0]); // bank select LSB
        setup(vec![0xC0 | channel, project.kit.min(127)]); // the kit
        setup(vec![0xB0 | channel, 7, 100]); // volume
        setup(vec![0xB0 | channel, 10, 64]); // pan, centre
        setup(vec![0xB0 | channel, 91, 40]); // reverb
        setup(vec![0xB0 | channel, 93, 0]); // chorus
    }

    let mut start = TICKS_PER_BAR;
    for part in parts {
        let end = start + part.steps as u32 * TICKS_PER_STEP;
        events.push((
            start,
            Order::Marker,
            meta(0x06, marker(part.section).as_bytes()),
        ));
        let steps = part.first..part.first + part.steps;
        for note in pattern_notes(project, part.pattern, steps, TICKS_PER_STEP) {
            let tick = start + note.tick;
            events.push((
                tick,
                Order::NoteOn,
                vec![0x90 | RHYTHM, note.key, note.velocity],
            ));
            // The note off, as a note on with velocity 0, as in the file the SX720 played.
            let off = (tick + NOTE_TICKS).min(end);
            events.push((off, Order::NoteOff, vec![0x90 | RHYTHM, note.key, 0]));
        }
        start = end;
    }
    // A stable sort keeps rows in order among events of the same tick and kind.
    events.sort_by_key(|&(tick, order, _)| (tick, order));
    smf(TICKS_PER_BEAT, &events, start)
}

/// The style's name in printable ASCII, which is what the keyboard shows. An accented Latin
/// letter becomes its plain letter, and any other character outside printable ASCII becomes
/// `_`. A name with nothing but spaces becomes "TickTapDrum".
fn style_name(name: &str) -> String {
    if name.trim().is_empty() {
        return DEFAULT_NAME.to_owned();
    }
    name.chars()
        .map(|c| {
            if (' '..='~').contains(&c) {
                c
            } else {
                plain_letter(c).unwrap_or('_')
            }
        })
        .collect()
}

/// The plain letter under an accented Latin letter, such as `a` for `á`.
fn plain_letter(c: char) -> Option<char> {
    const ACCENTED: &[(&str, char)] = &[
        ("ÀÁÂÃÄÅĀĂĄ", 'A'),
        ("àáâãäåāăą", 'a'),
        ("ÇĆĈĊČ", 'C'),
        ("çćĉċč", 'c'),
        ("ĎĐ", 'D'),
        ("ďđ", 'd'),
        ("ÈÉÊËĒĔĖĘĚ", 'E'),
        ("èéêëēĕėęě", 'e'),
        ("ĜĞĠĢ", 'G'),
        ("ĝğġģ", 'g'),
        ("ÌÍÎÏĨĪĬĮİ", 'I'),
        ("ìíîïĩīĭįı", 'i'),
        ("ĹĻĽĿŁ", 'L'),
        ("ĺļľŀł", 'l'),
        ("ÑŃŅŇ", 'N'),
        ("ñńņň", 'n'),
        ("ÒÓÔÕÖØŌŎŐ", 'O'),
        ("òóôõöøōŏő", 'o'),
        ("ŔŖŘ", 'R'),
        ("ŕŗř", 'r'),
        ("ŚŜŞŠ", 'S'),
        ("śŝşš", 's'),
        ("ŢŤŦ", 'T'),
        ("ţťŧ", 't'),
        ("ÙÚÛÜŨŪŬŮŰŲ", 'U'),
        ("ùúûüũūŭůűų", 'u'),
        ("ÝŸŶ", 'Y'),
        ("ýÿŷ", 'y'),
        ("ŹŻŽ", 'Z'),
        ("źżž", 'z'),
    ];
    ACCENTED
        .iter()
        .find(|(letters, _)| letters.contains(c))
        .map(|&(_, plain)| plain)
}
