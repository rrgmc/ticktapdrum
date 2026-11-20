//! The save file: a project written as JSON.
//!
//! The layout follows the design's data model. Rows get an id, which is their position plus one
//! at save time. Each pattern stores only the steps that are on, as `[step, velocity]` pairs per
//! row id, and that includes the steps of hidden bars, so hidden bars survive saving and loading.
//!
//! Loading is forgiving where nothing is lost: numbers out of range are clamped, steps for rows
//! or positions that do not exist are skipped, and unknown fields are ignored. It refuses a file
//! where accepting it would lose or invent data, such as two patterns with the same letter.

use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::io;

use serde::ser::SerializeMap;
use serde::{Deserialize, Serialize, Serializer};
use serde_json::Value;
use serde_json::ser::Formatter;

use crate::{
    DEFAULT_KIT, DEFAULT_TEMPO, DEFAULT_VOLUME, MAX_BARS, MAX_PATTERNS, MAX_ROWS, MAX_SONG,
    MAX_STEPS, MAX_SWING, Pattern, Project, Row, StepLevels, drum_name,
};

/// The file format this version writes. Files with a higher number are refused.
pub const FILE_FORMAT: u32 = 1;
/// The extension of a saved project, without the dot.
pub const FILE_EXTENSION: &str = "drums";

/// The slowest tempo a file can set, in beats per minute.
const MIN_TEMPO: f64 = 40.0;
/// The fastest tempo a file can set, in beats per minute.
const MAX_TEMPO: f64 = 240.0;
/// The highest MIDI velocity.
const MAX_VELOCITY: i64 = 127;
/// The sound bank every project uses for now.
const SOUNDFONT_KIND: &str = "colombogmgs2-17.11";

/// Why a file could not be loaded.
///
/// The `Display` text is English, for logs and tests. An interface shows its own translated
/// text instead, chosen by the variant and filled in from its fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadError {
    /// The text is not JSON. Holds the parser's description of the problem.
    NotJson(String),
    /// The file has a format number higher than [`FILE_FORMAT`].
    NewerFormat(u32),
    /// The file is JSON but does not describe a valid project.
    Invalid(InvalidReason),
}

/// What makes a JSON file not a valid project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InvalidReason {
    /// The JSON is not an object.
    NotAnObject,
    /// The object has no `format` field.
    FormatMissing,
    /// The format number is 0.
    FormatZero,
    /// The format is not a whole number.
    FormatNotWholeNumber,
    /// A field is missing or has the wrong type. Holds the parser's description of the problem.
    Layout(String),
    /// The project has no drum rows.
    NoRows,
    /// The project has more drum rows than allowed.
    TooManyRows { count: usize, max: usize },
    /// The project has no patterns.
    NoPatterns,
    /// The project has more patterns than allowed.
    TooManyPatterns { count: usize, max: usize },
    /// The song has more entries than allowed.
    SongTooLong { count: usize, max: usize },
    /// Two drum rows share this id.
    DuplicateRowId(u64),
    /// A pattern's name is not one letter from A to Z. Holds the name.
    BadPatternName(String),
    /// Two patterns share this name.
    DuplicatePattern(String),
    /// The song plays a pattern the file does not have. Holds the pattern's name.
    SongPatternMissing(String),
}

impl fmt::Display for InvalidReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotAnObject => write!(f, "it does not hold a project"),
            Self::FormatMissing => write!(f, "the file format number is missing"),
            Self::FormatZero => write!(f, "the file format number is 0"),
            Self::FormatNotWholeNumber => {
                write!(f, "the file format number is not a whole number")
            }
            Self::Layout(detail) => write!(f, "{detail}"),
            Self::NoRows => write!(f, "the project has no drum rows"),
            Self::TooManyRows { count, max } => write!(
                f,
                "the project has {count} drum rows, and the most allowed is {max}"
            ),
            Self::NoPatterns => write!(f, "the project has no patterns"),
            Self::TooManyPatterns { count, max } => write!(
                f,
                "the project has {count} patterns, and the most allowed is {max}"
            ),
            Self::SongTooLong { count, max } => write!(
                f,
                "the song has {count} entries, and the most allowed is {max}"
            ),
            Self::DuplicateRowId(id) => write!(f, "two drum rows have the id {id}"),
            Self::BadPatternName(name) => write!(
                f,
                "\"{name}\" is not a pattern name, which must be one letter from A to Z"
            ),
            Self::DuplicatePattern(name) => write!(f, "two patterns are named {name}"),
            Self::SongPatternMissing(name) => write!(
                f,
                "the song plays pattern \"{name}\", which is not in the file"
            ),
        }
    }
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotJson(detail) => write!(
                f,
                "This file is not a TickTapDrum project. It could not be read ({detail})."
            ),
            Self::NewerFormat(format) => write!(
                f,
                "This file was made by a newer version of TickTapDrum (file format \
                 {format}). Update the app to open it."
            ),
            Self::Invalid(reason) => {
                write!(f, "This file is damaged or incomplete: {reason}.")
            }
        }
    }
}

impl std::error::Error for LoadError {}

/// Writes a project as pretty-printed JSON. Fields always come in the same order, so two saves
/// of the same project give the same text.
pub fn to_json(project: &Project) -> String {
    let file = FileOut {
        format: FILE_FORMAT,
        tempo_bpm: project.tempo,
        swing: project.swing,
        step_levels: match project.step_levels {
            StepLevels::OnOff => "on_off",
            StepLevels::Soft => "soft",
            StepLevels::LoudAndSoft => "loud_and_soft",
        },
        soundfont: SoundfontOut {
            kind: SOUNDFONT_KIND,
        },
        kit_program: project.kit,
        rows: project
            .rows
            .iter()
            .enumerate()
            .map(|(index, row)| RowOut {
                id: index + 1,
                name: &row.name,
                key: row.key,
                volume: row.volume,
                mute: row.mute,
                solo: row.solo,
            })
            .collect(),
        patterns: project.patterns.iter().map(pattern_out).collect(),
        song: SongOut {
            order: project
                .song
                .iter()
                .map(|&slot| project.slot_name(slot).to_string())
                .collect(),
            looping: project.song_loop,
        },
    };
    let mut text = Vec::new();
    let mut serializer = serde_json::Serializer::with_formatter(&mut text, FileFormatter::new());
    file.serialize(&mut serializer)
        .expect("a project always serializes");
    text.push(b'\n');
    String::from_utf8(text).expect("serde_json writes UTF-8")
}

/// Reads a project written by [`to_json`], or by a hand that followed the same layout.
pub fn from_json(text: &str) -> Result<Project, LoadError> {
    let value: Value = serde_json::from_str(text).map_err(|e| LoadError::NotJson(e.to_string()))?;
    check_format(&value)?;
    let file: FileIn = serde_json::from_value(value)
        .map_err(|e| LoadError::Invalid(InvalidReason::Layout(e.to_string())))?;
    build(file)
}

/// Checks the format number before anything else, so a newer file is reported as newer even
/// when its layout has changed.
fn check_format(value: &Value) -> Result<(), LoadError> {
    let Some(object) = value.as_object() else {
        return Err(LoadError::Invalid(InvalidReason::NotAnObject));
    };
    let Some(format) = object.get("format") else {
        return Err(LoadError::Invalid(InvalidReason::FormatMissing));
    };
    match format.as_u64() {
        Some(0) => Err(LoadError::Invalid(InvalidReason::FormatZero)),
        Some(n) if n > u64::from(FILE_FORMAT) => {
            Err(LoadError::NewerFormat(u32::try_from(n).unwrap_or(u32::MAX)))
        }
        Some(_) => Ok(()),
        None => Err(LoadError::Invalid(InvalidReason::FormatNotWholeNumber)),
    }
}

/// The engine slot of a pattern letter, A to Z.
fn letter_slot(id: &str) -> Option<usize> {
    let mut chars = id.chars();
    match (chars.next(), chars.next()) {
        (Some(c @ 'A'..='Z'), None) => Some(c as usize - 'A' as usize),
        _ => None,
    }
}

/// Turns the parsed file into a project, applying the clamping and validity rules.
fn build(file: FileIn) -> Result<Project, LoadError> {
    if file.rows.is_empty() {
        return Err(LoadError::Invalid(InvalidReason::NoRows));
    }
    if file.rows.len() > MAX_ROWS {
        return Err(LoadError::Invalid(InvalidReason::TooManyRows {
            count: file.rows.len(),
            max: MAX_ROWS,
        }));
    }
    if file.patterns.is_empty() {
        return Err(LoadError::Invalid(InvalidReason::NoPatterns));
    }
    if file.patterns.len() > MAX_PATTERNS {
        return Err(LoadError::Invalid(InvalidReason::TooManyPatterns {
            count: file.patterns.len(),
            max: MAX_PATTERNS,
        }));
    }
    if file.song.order.len() > MAX_SONG {
        return Err(LoadError::Invalid(InvalidReason::SongTooLong {
            count: file.song.order.len(),
            max: MAX_SONG,
        }));
    }

    // Row ids map to positions. The file's ids need not be 1, 2, 3, but they must be unique.
    let mut row_of_id = HashMap::new();
    let mut rows = Vec::with_capacity(file.rows.len());
    for (index, row) in file.rows.into_iter().enumerate() {
        if row_of_id.insert(row.id, index).is_some() {
            return Err(LoadError::Invalid(InvalidReason::DuplicateRowId(row.id)));
        }
        rows.push(Row {
            name: row.name.unwrap_or_else(|| drum_name(row.key)),
            key: row.key,
            volume: row.volume.round().clamp(0.0, 100.0) as u8,
            mute: row.mute,
            solo: row.solo,
        });
    }

    let mut patterns: Vec<Pattern> = Vec::with_capacity(file.patterns.len());
    for pattern in file.patterns {
        let slot = letter_slot(&pattern.id)
            .ok_or_else(|| LoadError::Invalid(InvalidReason::BadPatternName(pattern.id.clone())))?;
        if patterns.iter().any(|p| p.slot == slot) {
            return Err(LoadError::Invalid(InvalidReason::DuplicatePattern(
                pattern.id,
            )));
        }
        let mut steps = vec![[0; MAX_STEPS]; rows.len()];
        for (id, pairs) in &pattern.steps {
            // A step list for a row that does not exist has nowhere to go; skip it.
            let Some(&row) = id.parse::<u64>().ok().and_then(|id| row_of_id.get(&id)) else {
                continue;
            };
            for &(step, velocity) in pairs {
                let Ok(step) = usize::try_from(step) else {
                    continue;
                };
                if step >= MAX_STEPS {
                    continue;
                }
                let velocity = velocity.clamp(0, MAX_VELOCITY) as u8;
                if velocity > 0 {
                    steps[row][step] = velocity;
                }
            }
        }
        // A file's letter is also the slot it loads into, so every letter has a slot.
        patterns.push(Pattern {
            slot,
            name: crate::letter(slot),
            bars: pattern.bar_count.clamp(1, MAX_BARS as i64) as usize,
            steps,
        });
    }

    let song = file
        .song
        .order
        .iter()
        .map(|id| {
            letter_slot(id)
                .filter(|&slot| patterns.iter().any(|p| p.slot == slot))
                .ok_or_else(|| LoadError::Invalid(InvalidReason::SongPatternMissing(id.clone())))
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(Project {
        rows,
        patterns,
        song,
        song_loop: file.song.looping,
        tempo: file.tempo_bpm.clamp(MIN_TEMPO, MAX_TEMPO) as f32,
        kit: file.kit_program,
        // Files from before swing existed hold 0.0 here, which is the default anyway.
        swing: file.swing.clamp(0.0, f64::from(MAX_SWING)) as f32,
        step_levels: match file.step_levels.as_deref() {
            Some("on_off") => StepLevels::OnOff,
            Some("soft") => StepLevels::Soft,
            Some("loud_and_soft") => StepLevels::LoudAndSoft,
            // Missing, or a choice from a newer app that this one does not know.
            _ => StepLevels::OnOff,
        },
    })
}

fn pattern_out(pattern: &Pattern) -> PatternOut {
    let steps = pattern
        .steps
        .iter()
        .enumerate()
        .filter_map(|(row, steps)| {
            let pairs: Vec<(usize, u8)> = steps
                .iter()
                .enumerate()
                .filter(|&(_, &velocity)| velocity > 0)
                .map(|(step, &velocity)| (step, velocity))
                .collect();
            (!pairs.is_empty()).then_some((row + 1, pairs))
        })
        .collect();
    PatternOut {
        id: pattern.name().to_string(),
        bar_count: pattern.bars,
        steps: StepsOut(steps),
    }
}

// What is written. The field order here is the order in the file.

#[derive(Serialize)]
struct FileOut<'a> {
    format: u32,
    tempo_bpm: f32,
    swing: f32,
    step_levels: &'static str,
    soundfont: SoundfontOut,
    kit_program: u8,
    rows: Vec<RowOut<'a>>,
    patterns: Vec<PatternOut>,
    song: SongOut,
}

#[derive(Serialize)]
struct SoundfontOut {
    kind: &'static str,
}

#[derive(Serialize)]
struct RowOut<'a> {
    id: usize,
    name: &'a str,
    key: u8,
    volume: u8,
    mute: bool,
    solo: bool,
}

#[derive(Serialize)]
struct PatternOut {
    id: String,
    bar_count: usize,
    steps: StepsOut,
}

/// Row id and its `(step, velocity)` pairs, in row order. Written as a JSON object keyed by the
/// row id, so row 10 comes after row 9 rather than after row 1 as a sorted map would put it.
struct StepsOut(Vec<(usize, Vec<(usize, u8)>)>);

impl Serialize for StepsOut {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(self.0.len()))?;
        for (id, pairs) in &self.0 {
            map.serialize_entry(&id.to_string(), pairs)?;
        }
        map.end()
    }
}

#[derive(Serialize)]
struct SongOut {
    order: Vec<String>,
    #[serde(rename = "loop")]
    looping: bool,
}

// What is read. Fields not listed here, such as `soundfont`, are ignored. Numbers that get
// clamped are read as wide types, so an out-of-range value is clamped instead of refused.

#[derive(Deserialize)]
struct FileIn {
    #[serde(default = "default_tempo")]
    tempo_bpm: f64,
    #[serde(default)]
    swing: f64,
    #[serde(default)]
    step_levels: Option<String>,
    #[serde(default = "default_kit")]
    kit_program: u8,
    rows: Vec<RowIn>,
    patterns: Vec<PatternIn>,
    #[serde(default)]
    song: SongIn,
}

#[derive(Deserialize)]
struct RowIn {
    id: u64,
    #[serde(default)]
    name: Option<String>,
    key: u8,
    #[serde(default = "default_volume")]
    volume: f64,
    #[serde(default)]
    mute: bool,
    #[serde(default)]
    solo: bool,
}

#[derive(Deserialize)]
struct PatternIn {
    id: String,
    #[serde(default = "default_bar_count")]
    bar_count: i64,
    #[serde(default)]
    steps: BTreeMap<String, Vec<(i64, i64)>>,
}

#[derive(Deserialize)]
struct SongIn {
    #[serde(default)]
    order: Vec<String>,
    #[serde(rename = "loop", default = "default_loop")]
    looping: bool,
}

impl Default for SongIn {
    fn default() -> Self {
        Self {
            order: Vec::new(),
            looping: default_loop(),
        }
    }
}

fn default_tempo() -> f64 {
    f64::from(DEFAULT_TEMPO)
}

fn default_kit() -> u8 {
    DEFAULT_KIT
}

fn default_volume() -> f64 {
    f64::from(DEFAULT_VOLUME)
}

fn default_bar_count() -> i64 {
    1
}

fn default_loop() -> bool {
    true
}

/// Pretty-prints with two-space indents, like serde_json's own pretty printer, except that
/// arrays nested three or more levels deep stay on one line. Those are the song order, each
/// row's step list and each `[step, velocity]` pair, which would otherwise take one line per
/// number.
struct FileFormatter {
    depth: usize,
    has_value: bool,
}

/// Arrays at this nesting depth or deeper are written on one line.
const INLINE_DEPTH: usize = 3;

impl FileFormatter {
    fn new() -> Self {
        Self {
            depth: 0,
            has_value: false,
        }
    }

    fn inline(&self) -> bool {
        self.depth >= INLINE_DEPTH
    }

    fn newline<W: ?Sized + io::Write>(&self, writer: &mut W) -> io::Result<()> {
        writer.write_all(b"\n")?;
        for _ in 0..self.depth {
            writer.write_all(b"  ")?;
        }
        Ok(())
    }
}

impl Formatter for FileFormatter {
    fn begin_array<W: ?Sized + io::Write>(&mut self, writer: &mut W) -> io::Result<()> {
        self.depth += 1;
        self.has_value = false;
        writer.write_all(b"[")
    }

    fn end_array<W: ?Sized + io::Write>(&mut self, writer: &mut W) -> io::Result<()> {
        let inline = self.inline();
        self.depth -= 1;
        if self.has_value && !inline {
            self.newline(writer)?;
        }
        writer.write_all(b"]")
    }

    fn begin_array_value<W: ?Sized + io::Write>(
        &mut self,
        writer: &mut W,
        first: bool,
    ) -> io::Result<()> {
        if !first {
            writer.write_all(if self.inline() {
                b", ".as_slice()
            } else {
                b","
            })?;
        }
        if self.inline() {
            Ok(())
        } else {
            self.newline(writer)
        }
    }

    fn end_array_value<W: ?Sized + io::Write>(&mut self, _writer: &mut W) -> io::Result<()> {
        self.has_value = true;
        Ok(())
    }

    fn begin_object<W: ?Sized + io::Write>(&mut self, writer: &mut W) -> io::Result<()> {
        self.depth += 1;
        self.has_value = false;
        writer.write_all(b"{")
    }

    fn end_object<W: ?Sized + io::Write>(&mut self, writer: &mut W) -> io::Result<()> {
        self.depth -= 1;
        if self.has_value {
            self.newline(writer)?;
        }
        writer.write_all(b"}")
    }

    fn begin_object_key<W: ?Sized + io::Write>(
        &mut self,
        writer: &mut W,
        first: bool,
    ) -> io::Result<()> {
        if !first {
            writer.write_all(b",")?;
        }
        self.newline(writer)
    }

    fn begin_object_value<W: ?Sized + io::Write>(&mut self, writer: &mut W) -> io::Result<()> {
        writer.write_all(b": ")
    }

    fn end_object_value<W: ?Sized + io::Write>(&mut self, _writer: &mut W) -> io::Result<()> {
        self.has_value = true;
        Ok(())
    }
}

#[cfg(test)]
#[path = "file_tests.rs"]
mod tests;
