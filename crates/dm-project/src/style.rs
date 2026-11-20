//! Keyboard style export: the project as a drum-only Yamaha arranger style.
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

use crate::midi::{meta, pattern_notes, smf, tempo_bytes};
use crate::{Pattern, Project, STEPS_PER_BAR};

/// The extension of an exported style file, without the dot.
pub const STYLE_EXTENSION: &str = "sty";
/// The file's time resolution: ticks in one beat.
const TICKS_PER_BEAT: u16 = 1920;
/// Ticks in one step. A beat holds four steps.
const TICKS_PER_STEP: u32 = TICKS_PER_BEAT as u32 / 4;
const TICKS_PER_BAR: u32 = TICKS_PER_STEP * STEPS_PER_BAR as u32;
/// How long each note is held: half a step, as in the MIDI export.
const NOTE_TICKS: u32 = TICKS_PER_STEP / 2;
/// Channel 10 (Rhythm) and channel 9 (Sub-Rhythm), counted from 0 as the status byte does. The
/// notes go on Rhythm. Sub-Rhythm gets the same setup and no notes, as in the file the SX720
/// played.
const RHYTHM: u8 = 9;
const SUB_RHYTHM: u8 = 8;
/// The name used when the project's name has nothing printable in it.
const DEFAULT_NAME: &str = "TickTapDrum";

const INTRO: &str = "Intro A";
const MAINS: [&str; 4] = ["Main A", "Main B", "Main C", "Main D"];
const FILLS: [&str; 4] = ["Fill In AA", "Fill In BB", "Fill In CC", "Fill In DD"];
const BREAK: &str = "Fill In BA";
const ENDING: &str = "Ending A";

/// Which pattern plays each section of the style, by engine slot ([`Pattern::slot`]). `None`
/// leaves the section out.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StyleSections {
    /// Intro A.
    pub intro: Option<usize>,
    /// Main A to D. Main A is required.
    pub mains: [Option<usize>; 4],
    /// Fill In AA to DD, one for each main. Each must be a one-bar pattern. A main without a fill
    /// uses its own last bar as its fill.
    pub fills: [Option<usize>; 4],
    /// The Break, Fill In BA. It must be a one-bar pattern.
    pub fill_break: Option<usize>,
    /// Ending A.
    pub ending: Option<usize>,
}

/// Why a style cannot be written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StyleProblem {
    /// Main A has no pattern. Some keyboards refuse a style without it.
    NoMainA,
    /// A fill or the Break uses a pattern longer than one bar.
    NotOneBar,
    /// A section names a slot that holds no pattern.
    NoSuchPattern,
}

impl StyleSections {
    /// The app's suggestion, from the song.
    ///
    /// The song's patterns are taken in order, each once; without a song, the project's patterns
    /// in their own order. A one-bar pattern that comes right after a main which has no fill yet
    /// becomes that main's fill. Any other pattern becomes the next free main, up to Main D; the
    /// rest are left out. The Intro, the Break and the Ending start empty.
    pub fn suggested(project: &Project) -> Self {
        let order: Vec<usize> = if project.song.is_empty() {
            project.patterns.iter().map(|p| p.slot).collect()
        } else {
            project.song.clone()
        };
        let mut sections = Self::default();
        let mut seen = Vec::new();
        let mut previous = None;
        for slot in order {
            let Some(pattern) = find(project, slot) else {
                continue;
            };
            if !seen.contains(&slot) {
                seen.push(slot);
                let after_main = previous
                    .and_then(|before| sections.mains.iter().position(|&m| m == Some(before)));
                match after_main {
                    Some(main) if pattern.bars() == 1 && sections.fills[main].is_none() => {
                        sections.fills[main] = Some(slot);
                    }
                    _ => {
                        if let Some(free) = sections.mains.iter_mut().find(|m| m.is_none()) {
                            *free = Some(slot);
                        }
                    }
                }
            }
            previous = Some(slot);
        }
        sections
    }

    /// The first problem that stops the export, if any. A fill whose main is left out is not
    /// written, so it is not checked.
    pub fn problem(&self, project: &Project) -> Option<StyleProblem> {
        self.sections(project).err()
    }

    /// The sections to write, in file order: marker, pattern and the steps that play.
    fn sections<'a>(&self, project: &'a Project) -> Result<Vec<Section<'a>>, StyleProblem> {
        if self.mains[0].is_none() {
            return Err(StyleProblem::NoMainA);
        }
        let pattern = |slot: usize| find(project, slot).ok_or(StyleProblem::NoSuchPattern);
        let one_bar = |slot: usize| {
            let pattern = pattern(slot)?;
            if pattern.bars() == 1 {
                Ok(pattern)
            } else {
                Err(StyleProblem::NotOneBar)
            }
        };
        let mut sections = Vec::new();
        if let Some(slot) = self.intro {
            sections.push(Section::whole(INTRO, pattern(slot)?));
        }
        for (index, main) in self.mains.iter().enumerate() {
            let Some(main) = *main else {
                continue;
            };
            let main = pattern(main)?;
            sections.push(Section::whole(MAINS[index], main));
            let fill = match self.fills[index] {
                Some(fill) => Section::whole(FILLS[index], one_bar(fill)?),
                None => {
                    let last = main.length() - STEPS_PER_BAR;
                    Section {
                        marker: FILLS[index],
                        pattern: main,
                        first: last,
                        steps: STEPS_PER_BAR,
                    }
                }
            };
            sections.push(fill);
        }
        if let Some(slot) = self.fill_break {
            sections.push(Section::whole(BREAK, one_bar(slot)?));
        }
        if let Some(slot) = self.ending {
            sections.push(Section::whole(ENDING, pattern(slot)?));
        }
        Ok(sections)
    }
}

/// One section of the file: its marker and the steps of a pattern it plays.
struct Section<'a> {
    marker: &'static str,
    pattern: &'a Pattern,
    /// The first step, at the start of a bar.
    first: usize,
    /// The number of steps, a whole number of bars.
    steps: usize,
}

impl<'a> Section<'a> {
    /// A section that plays every playing bar of the pattern.
    fn whole(marker: &'static str, pattern: &'a Pattern) -> Self {
        Self {
            marker,
            pattern,
            first: 0,
            steps: pattern.length(),
        }
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

/// Writes the project as a drum-only Yamaha style, with `name` as the style's name.
///
/// Refused with the first [`StyleProblem`] of `sections`. Each section plays its pattern's
/// playing bars; hidden bars are left out. A main without a fill gets its own last bar as the
/// fill. Mute, solo, row volume and swing apply as in [`crate::to_midi`]. A note that would ring
/// past the end of its section is cut off there.
pub fn to_style(
    project: &Project,
    sections: &StyleSections,
    name: &str,
) -> Result<Vec<u8>, StyleProblem> {
    let sections = sections.sections(project)?;
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
    for section in &sections {
        let end = start + section.steps as u32 * TICKS_PER_STEP;
        events.push((start, Order::Marker, meta(0x06, section.marker.as_bytes())));
        let steps = section.first..section.first + section.steps;
        for note in pattern_notes(project, section.pattern, steps, TICKS_PER_STEP) {
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
    Ok(smf(TICKS_PER_BEAT, &events, start))
}

/// The pattern in an engine slot.
fn find(project: &Project, slot: usize) -> Option<&Pattern> {
    project.patterns.iter().find(|p| p.slot == slot)
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

#[cfg(test)]
#[path = "style_tests.rs"]
mod tests;
