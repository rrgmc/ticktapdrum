//! Turns pasted plain-text (ASCII) drum tabs into bars on TickTapDrum's 16-step grid.
//!
//! A tab is a text with "systems": runs of lines that each start with a drum label and hold a row
//! of hit symbols split into bars by `|`, all aligned under each other:
//!
//! ```text
//! C |x---------------|----------------|
//! HH|--x-x-x-x-x-x-x-|x-x-x-x-x-x-x-x-|
//! S |----o-------o---|----o-------o---|
//! B |o-------o-o-----|o-------o-o-----|  x4
//! ```
//!
//! The app is for beginners, so the import never asks a question. It reads what it can recognise
//! on its own and refuses the rest with a reason ([`Refusal`]). Lyrics, guitar tabs, legends and
//! comments between systems are skipped.
//!
//! Each bar's width is its grid: 16 columns are 16ths, 8 are 8ths, 4 are quarters and 32 are
//! 32nds. A bar one column wider than that, with an empty first or last column in every row, has
//! a spacer column, which is trimmed. A bar one column short is padded. Any other width is
//! refused. A legend in the tab, such as `T = Tambourine`, wins over the built-in label table.

use std::collections::{BTreeMap, BTreeSet};

use dm_project::{BarHit, Change, Project, STEPS_PER_BAR};

mod labels;
mod read;

use read::{DrumLine, drum_line, is_count_line, is_rest, read_bar, repeat_mark, step_of};

/// Bar widths, in columns, that fit the grid.
const STANDARD: [usize; 4] = [4, 8, 16, 32];
/// The most plays a repeat mark may ask for. A larger number is more likely a typo.
const MAX_REPEAT: usize = 16;
/// Labels that name guitar or bass strings. A system with three of them is not drums.
const STRING_LABELS: [&str; 8] = ["e", "E", "A", "a", "D", "d", "G", "g"];

/// One hit: General MIDI drum key, step within its bar (0..16), velocity 1..=127.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hit {
    pub key: u8,
    pub step: usize,
    pub velocity: u8,
}

/// What a paste holds, after repeats: bars of hits, and what had to be dropped.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Tab {
    pub bars: Vec<Vec<Hit>>,
    /// Tempo stated in the text, if any (informational; the app keeps the project tempo).
    pub tempo: Option<f32>,
    /// Labels of rows that were dropped because no drum is known for them.
    pub dropped_labels: Vec<String>,
    /// Hits merged into another on the same step and drum (32nd notes, flams).
    pub merged: usize,
}

/// Why a paste cannot be imported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// No drum tab found in the text.
    NoTab,
    /// Bars of widths the grid cannot hold (triplets or miscounted bars); the widths seen,
    /// sorted, deduplicated.
    OddBars(Vec<usize>),
    /// Rows whose drums are unknown hold more than 10% of the hits; their labels.
    UnknownDrums(Vec<String>),
    /// The rows of a system do not have the same number of bars.
    Misaligned,
}

/// What [`parse`] gathers while it reads the systems one by one.
#[derive(Default)]
struct Reader {
    bars: Vec<Vec<Hit>>,
    /// Drum systems that hold at least one hit symbol.
    systems: usize,
    misaligned: bool,
    odd_widths: BTreeSet<usize>,
    /// Hits on rows with a known drum, and on rows without one, as written (before repeats).
    known_hits: usize,
    unknown_hits: usize,
    unknown_labels: Vec<String>,
    merged: usize,
}

/// Reads pasted text. Refuses when the result would need guessing beyond the rules below.
///
/// 1. The text needs a drum system with at least one hit, else [`Refusal::NoTab`].
/// 2. Rows whose label names no drum are dropped if they hold at most 10% of the hits, else
///    [`Refusal::UnknownDrums`].
/// 3. After trimming spacers, padding short bars and copying `%` bars, every bar must be 4, 8,
///    16 or 32 columns wide, else [`Refusal::OddBars`].
/// 4. The rows of each system must have the same number of bars, else [`Refusal::Misaligned`].
/// 5. Repeat marks such as `x4` give total plays, at most 16. The bars come out repeated.
pub fn parse(text: &str) -> Result<Tab, Refusal> {
    let legend = labels::parse_legend(text);
    let normalized: Vec<String> = text.lines().map(read::normalize).collect();
    let lines: Vec<&str> = normalized.iter().map(String::as_str).collect();
    let mut reader = Reader::default();
    let mut i = 0;
    while i < lines.len() {
        let Some(first) = (!is_count_line(lines[i]))
            .then(|| drum_line(lines[i]))
            .flatten()
        else {
            i += 1;
            continue;
        };
        let mut system = vec![first];
        i += 1;
        while i < lines.len() {
            if is_count_line(lines[i]) {
                i += 1;
                continue;
            }
            match drum_line(lines[i]) {
                Some(line) => {
                    system.push(line);
                    i += 1;
                }
                None => break,
            }
        }
        // A repeat mark may also sit alone on the line after the system.
        let after = lines
            .get(i)
            .and_then(|l| repeat_mark(l).filter(|_| l.trim().len() < 30));
        reader.add_system(system, after, &legend);
    }

    if reader.systems == 0 {
        return Err(Refusal::NoTab);
    }
    if reader.misaligned {
        return Err(Refusal::Misaligned);
    }
    if !reader.odd_widths.is_empty() {
        return Err(Refusal::OddBars(reader.odd_widths.into_iter().collect()));
    }
    let all = reader.known_hits + reader.unknown_hits;
    if reader.unknown_hits * 10 > all {
        return Err(Refusal::UnknownDrums(reader.unknown_labels));
    }
    Ok(Tab {
        bars: reader.bars,
        tempo: read::find_tempo(text),
        dropped_labels: reader.unknown_labels,
        merged: reader.merged,
    })
}

impl Reader {
    fn add_system(
        &mut self,
        mut system: Vec<DrumLine>,
        after: Option<usize>,
        legend: &BTreeMap<String, u8>,
    ) {
        let strings = system
            .iter()
            .filter(|l| STRING_LABELS.contains(&l.label.as_str()))
            .count();
        if strings >= 3 {
            return;
        }
        // A system without a single hit symbol is a stray line, not a groove.
        let has_hits = system
            .iter()
            .flat_map(|l| &l.bars)
            .any(|bar| !read_bar(0, bar).is_empty());
        if !has_hits {
            return;
        }
        self.systems += 1;

        // A row with no bar lines at all is cut into bars of 16.
        for line in &mut system {
            if !line.piped && line.bars.len() == 1 {
                let bar = &line.bars[0];
                if bar.len() > STEPS_PER_BAR && bar.len() % STEPS_PER_BAR == 0 {
                    line.bars = bar.chunks(STEPS_PER_BAR).map(<[char]>::to_vec).collect();
                }
            }
        }

        let bars = system[0].bars.len();
        if system.iter().any(|l| l.bars.len() != bars) {
            self.misaligned = true;
            return;
        }

        // `%` alone in a bar repeats the row's previous bar.
        for line in &mut system {
            for b in 1..line.bars.len() {
                let mut content = line.bars[b].iter().filter(|c| !is_rest(**c));
                if content.next() == Some(&'%') && content.next().is_none() {
                    line.bars[b] = line.bars[b - 1].clone();
                }
            }
        }

        // Fix widths one column off a standard width, bar by bar across the system.
        for b in 0..bars {
            fix_width(&mut system, b);
        }
        let odd: Vec<usize> = system
            .iter()
            .flat_map(|l| &l.bars)
            .map(Vec::len)
            .filter(|w| !STANDARD.contains(w))
            .collect();
        if !odd.is_empty() {
            self.odd_widths.extend(odd);
            return;
        }

        let repeat = system
            .iter()
            .filter_map(|l| l.repeat)
            .chain(after)
            .max()
            .unwrap_or(1)
            .clamp(1, MAX_REPEAT);

        let mut parsed: Vec<Vec<Hit>> = vec![Vec::new(); bars];
        for line in &system {
            let Some(key) = labels::row_key(&line.label, legend) else {
                let hits: usize = line.bars.iter().map(|bar| read_bar(0, bar).len()).sum();
                self.unknown_hits += hits;
                if hits > 0 && !self.unknown_labels.contains(&line.label) {
                    self.unknown_labels.push(line.label.clone());
                }
                continue;
            };
            for (bar, symbols) in line.bars.iter().enumerate() {
                for s in read_bar(key, symbols) {
                    self.known_hits += 1;
                    parsed[bar].push(Hit {
                        key: s.key,
                        step: step_of(s.column, symbols.len()),
                        velocity: s.velocity,
                    });
                }
            }
        }
        for bar in &mut parsed {
            self.merged += merge(bar);
        }
        for _ in 0..repeat {
            self.bars.extend(parsed.iter().cloned());
        }
    }
}

/// Fixes bar `b` of a system when it is one column off a standard width in every row: trims an
/// empty spacer column at the start or end, or pads a short bar with a rest.
fn fix_width(system: &mut [DrumLine], b: usize) {
    let width = system[0].bars[b].len();
    if STANDARD.contains(&width) || system.iter().any(|l| l.bars[b].len() != width) {
        return;
    }
    let all = |f: &dyn Fn(&Vec<char>) -> bool| system.iter().all(|l| f(&l.bars[b]));
    if STANDARD.contains(&(width - 1)) && all(&|bar| is_rest(bar[0])) {
        for line in system.iter_mut() {
            line.bars[b].remove(0);
        }
    } else if STANDARD.contains(&(width - 1)) && all(&|bar| is_rest(bar[width - 1])) {
        for line in system.iter_mut() {
            line.bars[b].pop();
        }
    } else if STANDARD.contains(&(width + 1)) {
        for line in system.iter_mut() {
            line.bars[b].push('-');
        }
    }
}

/// Merges hits on the same step and drum, keeping the louder. Returns how many went.
fn merge(bar: &mut Vec<Hit>) -> usize {
    let before = bar.len();
    bar.sort_by(|a, b| {
        (a.key, a.step)
            .cmp(&(b.key, b.step))
            .then(b.velocity.cmp(&a.velocity))
    });
    bar.dedup_by(|a, b| a.key == b.key && a.step == b.step);
    before - bar.len()
}

/// What adding a tab to a project did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Added {
    /// Index of the new pattern in project.patterns().
    pub index: usize,
    /// Bars in the new pattern (at most dm_project::MAX_BARS).
    pub bars: usize,
    /// Bars after the first MAX_BARS that were left out.
    pub bars_left_out: usize,
    /// Rows added to the project for drums it did not have.
    pub rows_added: usize,
    /// Drums left out because the project already has MAX_ROWS rows.
    pub drums_left_out: usize,
}

/// Why a tab could not be added to a project. Nothing changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AddError {
    /// The project already has dm_project::MAX_PATTERNS patterns.
    NoFreePattern,
}

/// Adds the tab to the project as one new pattern after the existing ones, using rows that
/// already play each drum and adding rows for drums the project lacks. Bars that are one shorter
/// unit repeated collapse to that unit (see [`dm_project::repeat_unit`]), and at most
/// `dm_project::MAX_BARS` bars are kept. Returns what it did and the changes for the audio engine
/// (in order). Does not touch tempo, swing, kit or the song.
pub fn add_to_project(project: &mut Project, tab: &Tab) -> Result<(Added, Vec<Change>), AddError> {
    let bars: Vec<Vec<BarHit>> = if tab.bars.is_empty() {
        vec![Vec::new()]
    } else {
        tab.bars
            .iter()
            .map(|bar| {
                bar.iter()
                    .map(|h| BarHit {
                        key: h.key,
                        step: h.step,
                        velocity: h.velocity,
                    })
                    .collect()
            })
            .collect()
    };
    let (imported, changes) = project
        .add_imported_pattern(&bars)
        .map_err(|_| AddError::NoFreePattern)?;
    let added = Added {
        index: imported.index,
        bars: imported.bars,
        bars_left_out: imported.bars_left_out,
        rows_added: imported.rows_added,
        drums_left_out: imported.drums_left_out,
    };
    Ok((added, changes))
}

#[cfg(test)]
mod tests;
