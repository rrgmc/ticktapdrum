//! Decides whether one MIDI file is a groove, and turns it into bars of hits on the 16-step grid.
//!
//! This follows the groove spike (`tools/groove-spike`). The checks run in a fixed order, and the
//! first that fails gives the refusal.

use std::collections::{BTreeMap, BTreeSet};

use dm_project::{BarHit, STEPS_PER_BAR};

use crate::Refusal;
use crate::midi::{self, MidiFile};

/// The largest MIDI file read, in bytes. Groove files are a few kilobytes.
pub const MAX_MIDI_BYTES: usize = 1024 * 1024;
/// The most bars a groove may have.
pub const MAX_GROOVE_BARS: usize = 16;
/// The tempo when a file gives none, as the MIDI standard says.
const DEFAULT_BPM: f64 = 120.0;
/// Tempos that differ by less than this count as the same.
const SAME_TEMPO: f64 = 0.01;
/// The tempo range a groove is given.
const MIN_BPM: f64 = 40.0;
const MAX_BPM: f64 = 240.0;
/// A note further than this from its grid point, in steps, is "off the grid": a triplet, a flam,
/// a roll or a badly late note. A quarter step is 31 ms at 120 BPM.
const FAR: f64 = 0.25;
/// The largest share of notes that may be off the grid.
const MAX_FAR_SHARE: f64 = 0.10;
/// The smallest share of notes that must be on drum-kit keys.
const MIN_KIT_SHARE: f64 = 0.90;
/// Keys of a drum kit's own drums and cymbals in General MIDI: kick to ride bell and beyond.
const KIT_KEYS: std::ops::RangeInclusive<u8> = 35..=59;
/// Hand percussion inside [`KIT_KEYS`]: tambourine, cowbell and vibraslap. They do not count as
/// kit keys. Some drum programs put their hi-hat on the tambourine key (54), so a groove mostly
/// on these keys is made for another program.
const PERCUSSION_IN_KIT_RANGE: [u8; 3] = [54, 56, 58];

/// Whether a key is one of a drum kit's own drums or cymbals.
fn is_kit_key(key: u8) -> bool {
    KIT_KEYS.contains(&key) && !PERCUSSION_IN_KIT_RANGE.contains(&key)
}
/// General MIDI's hand percussion above the kit: bongos to open triangle. In a style these count
/// as drum keys, since the kit's key map has already put each sound on its General MIDI key.
const STYLE_PERCUSSION_KEYS: std::ops::RangeInclusive<u8> = 60..=81;
/// Keys that General MIDI and the Roland GS extras give a drum sound.
const KNOWN_KEYS: std::ops::RangeInclusive<u8> = 27..=87;
/// The most notes the swing fit looks at. Real grooves have far fewer, so this only limits the
/// time spent on huge files.
const MAX_FIT_NOTES: usize = 4096;

/// What an accepted file becomes.
#[derive(Debug, Clone, PartialEq)]
pub struct Fitted {
    pub bpm: f32,
    pub bars: Vec<Vec<BarHit>>,
}

/// Where the notes come from. A style's notes have been through its kit's key map already, so
/// they are checked a little differently: see [`map_keys`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// An ordinary MIDI file.
    MidiFile,
    /// One section of a keyboard style.
    Style,
}

/// Reads and checks one MIDI file.
pub fn analyze(bytes: &[u8]) -> Result<Fitted, Refusal> {
    if bytes.len() > MAX_MIDI_BYTES {
        return Err(Refusal::NotMidi);
    }
    let file = midi::parse(bytes).ok_or(Refusal::NotMidi)?;
    fit(&file, Source::MidiFile)
}

/// Checks notes already read, and snaps them to the grid.
pub fn fit(file: &MidiFile, source: Source) -> Result<Fitted, Refusal> {
    if file.notes.is_empty() {
        return Err(Refusal::NoNotes);
    }
    let channels: BTreeSet<u8> = file.notes.iter().map(|n| n.channel).collect();
    if channels.len() > 1 {
        return Err(Refusal::SeveralChannels);
    }
    // No time signature means 4/4. Any other signature, at the start or later, is refused.
    if file.time_signatures.iter().any(|t| (t.1, t.2) != (4, 4)) {
        return Err(Refusal::NotFourFour);
    }
    let bpm = file.tempos.first().map_or(DEFAULT_BPM, |t| t.1);
    // A style's tempo is the one its setup bar sets; the keyboard player chooses the tempo, so a
    // change inside a section (an Ending slowing down, say) is ignored.
    if source == Source::MidiFile && file.tempos.iter().any(|t| (t.1 - bpm).abs() >= SAME_TEMPO) {
        return Err(Refusal::TempoChanges);
    }

    let keys = map_keys(file, source)?;

    // Positions in steps, from the start of the file.
    let step_ticks = f64::from(file.ppq) / 4.0;
    let raw: Vec<f64> = file
        .notes
        .iter()
        .map(|n| n.tick as f64 / step_ticks)
        .collect();
    let (swing, offset) = fit_swing(&raw);
    let positions: Vec<f64> = raw.iter().map(|&x| (x - offset).max(0.0)).collect();
    let snapped: Vec<(usize, f64)> = positions.iter().map(|&x| snap(x, swing)).collect();
    let far = snapped.iter().filter(|s| s.1 > FAR).count();
    if far as f64 > MAX_FAR_SHARE * snapped.len() as f64 {
        return Err(Refusal::OffGrid);
    }

    // The file lasts to its last note, or to its End of Track if that is later and on a bar line.
    // An End of Track between bar lines is padding, not the loop's length: the Groove MIDI
    // Dataset ends its files a fifth of a bar to a whole bar after the last note. Half a step of
    // slack keeps a last bar that ends a little early from counting as a new one.
    let end_steps = file.end_tick as f64 / step_ticks;
    let off_bar_line = (end_steps / STEPS_PER_BAR as f64).fract() * STEPS_PER_BAR as f64;
    let on_bar_line = off_bar_line <= 0.5 || off_bar_line >= STEPS_PER_BAR as f64 - 0.5;
    let total_steps = positions
        .iter()
        .copied()
        .fold(if on_bar_line { end_steps } else { 0.0 }, f64::max);
    let bars = (((total_steps - 0.5) / STEPS_PER_BAR as f64).ceil()).max(1.0);
    if bars > MAX_GROOVE_BARS as f64 {
        return Err(Refusal::TooLong);
    }
    let bars = bars as usize;

    // Two notes of one drum on one step become the louder one.
    let mut cells: BTreeMap<(usize, u8, usize), u8> = BTreeMap::new();
    for ((&(step, _), &key), note) in snapped.iter().zip(&keys).zip(&file.notes) {
        // A note that snaps past the end, such as a last hit played early for the next loop,
        // wraps to the start.
        let step = step % (bars * STEPS_PER_BAR);
        let cell = cells
            .entry((step / STEPS_PER_BAR, key, step % STEPS_PER_BAR))
            .or_insert(0);
        *cell = (*cell).max(note.velocity);
    }
    let mut out = vec![Vec::new(); bars];
    for ((bar, key, step), velocity) in cells {
        out[bar].push(BarHit {
            key,
            step,
            velocity,
        });
    }
    // Empty bars at either end are silence around the groove, such as a fill whose first hit
    // falls on the second bar line, and would loop as a gap. There is always a note, so a bar
    // stays.
    while out.last().is_some_and(Vec::is_empty) {
        out.pop();
    }
    let leading = out.iter().take_while(|bar| bar.is_empty()).count();
    out.drain(..leading);
    Ok(Fitted {
        bpm: bpm.round().clamp(MIN_BPM, MAX_BPM) as f32,
        bars: out,
    })
}

/// Maps every note's key to the General MIDI key the app plays, or refuses the file as made for
/// another drum program.
///
/// A style's keys are General MIDI already, so they skip the TD-11 mapping, and its hand
/// percussion (keys 60 to 81) counts as drum keys.
fn map_keys(file: &MidiFile, source: Source) -> Result<Vec<u8>, Refusal> {
    // Keys 22 and 26 are the hi-hat edge of the Roland TD-11, the kit Google's Groove MIDI
    // Dataset was recorded on. General MIDI has no drum there.
    let td11 = source == Source::MidiFile && file.notes.iter().any(|n| n.key == 22 || n.key == 26);
    let keys: Vec<u8> = file
        .notes
        .iter()
        .map(|n| if td11 { td11_key(n.key) } else { n.key })
        .collect();
    if keys.iter().any(|k| !KNOWN_KEYS.contains(k)) {
        return Err(Refusal::OtherDrumProgram);
    }
    // In a style, the tambourine, cowbell and vibraslap keys hold those sounds: the key map put
    // them there, so they count too.
    let counts = |k: u8| match source {
        Source::MidiFile => is_kit_key(k),
        Source::Style => KIT_KEYS.contains(&k) || STYLE_PERCUSSION_KEYS.contains(&k),
    };
    let kit = keys.iter().filter(|&&k| counts(k)).count();
    if (kit as f64) < MIN_KIT_SHARE * keys.len() as f64 {
        return Err(Refusal::OtherDrumProgram);
    }
    Ok(keys)
}

/// The General MIDI key for a key of the TD-11 layout, from the Groove MIDI Dataset's
/// documentation. Keys that mean the same in both pass through.
fn td11_key(key: u8) -> u8 {
    match key {
        40 => 38, // snare rim: a rimshot, not General MIDI's electric snare
        48 => 50, // tom 1 head (high)
        45 => 47, // tom 2 head (mid)
        58 => 43, // tom 3 rim: General MIDI 58 is the vibraslap
        26 => 46, // hi-hat open, edge
        22 => 42, // hi-hat closed, edge
        55 => 49, // crash 1 edge: General MIDI 55 is the splash
        52 => 57, // crash 2 edge: General MIDI 52 is the china
        59 => 51, // ride edge
        other => other,
    }
}

/// The swing (0 to 0.5) and constant offset (within a quarter step either way) that put the
/// notes closest to the swung grid. Swing is as the app defines it: the fraction of a step every
/// odd step is late. The offset is positive when the whole file plays late.
fn fit_swing(positions: &[f64]) -> (f64, f64) {
    // Spread the sample over the whole file when there are too many notes.
    let every = positions.len().div_ceil(MAX_FIT_NOTES).max(1);
    let sample: Vec<f64> = positions.iter().step_by(every).copied().collect();
    let error = |swing: f64, offset: f64| {
        sample
            .iter()
            .map(|&x| snap((x - offset).max(0.0), swing).1)
            .sum::<f64>()
            / sample.len().max(1) as f64
    };
    let straight = error(0.0, 0.0);
    let (mut swing, mut offset, mut best) = (0.0, 0.0, straight);
    for o in -12..=12 {
        let o = f64::from(o) / 50.0;
        for s in 0..=50 {
            let s = f64::from(s) / 100.0;
            let e = error(s, o);
            if e < best - 1e-9 {
                (swing, offset, best) = (s, o, e);
            }
        }
    }
    // A fit that barely helps is noise in a straight, on-time groove.
    if straight - best < 0.01 {
        (0.0, 0.0)
    } else {
        (swing, offset)
    }
}

/// The step a position snaps to on the swung grid, and the distance moved, both in steps. In
/// each pair of steps the grid points are at 0 and `1 + swing`. `x` is in straight steps and
/// not negative; `swing` is the fraction of a step every odd step is delayed, as in
/// `dm_project::Project::swing`. The step may be one past the last, for the caller to wrap.
pub fn snap(x: f64, swing: f64) -> (usize, f64) {
    let pair = (x / 2.0).floor();
    let r = x - pair * 2.0;
    let (offset, distance) = [(0, 0.0), (1, 1.0 + swing), (2, 2.0)]
        .into_iter()
        .map(|(o, p)| (o, (r - p).abs()))
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .expect("three candidates");
    (pair as usize * 2 + offset, distance)
}
