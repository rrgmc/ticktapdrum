//! Adding bars of hits from outside the app, such as a groove file or a pasted drum tab, as one
//! new pattern.
//!
//! An import is kept simple on purpose: it becomes one pattern of at most [`MAX_BARS`] bars, and
//! bars that are only one shorter unit repeated collapse to that unit, so 8 bars written as a
//! 2-bar groove four times become those 2 bars.

use crate::{Change, DrumFamily, MAX_BARS, MAX_ROWS, Project, Refused, STEPS_PER_BAR, drum_family};

/// Where a drum's family sorts among new rows: kick first, percussion last.
fn family_order(key: u8) -> u8 {
    match drum_family(key) {
        DrumFamily::Kick => 0,
        DrumFamily::Snare => 1,
        DrumFamily::HiHat => 2,
        DrumFamily::Tom => 3,
        DrumFamily::Cymbal => 4,
        DrumFamily::Percussion => 5,
    }
}

/// One hit to place: a General MIDI drum key, the step within its bar (0 to 15) and a velocity
/// from 1 to 127.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BarHit {
    pub key: u8,
    pub step: usize,
    pub velocity: u8,
}

/// What adding an import did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Imported {
    /// The new pattern's index in [`Project::patterns`].
    pub index: usize,
    /// Bars in the new pattern.
    pub bars: usize,
    /// Bars given, before repeats collapsed. Equal to `bars` plus `bars_left_out` when nothing
    /// collapsed.
    pub bars_given: usize,
    /// Bars of the repeating unit beyond [`MAX_BARS`] that were left out.
    pub bars_left_out: usize,
    /// Rows added for drums the project did not have.
    pub rows_added: usize,
    /// Drums left out because the project already had [`MAX_ROWS`] rows.
    pub drums_left_out: usize,
}

/// The length of the shortest unit that, repeated, gives exactly these bars. Bars compare by
/// which drums play on which steps; velocities are ignored, so a repeat with other accents still
/// counts. Bars that do not repeat give their own length.
pub fn repeat_unit(bars: &[Vec<BarHit>]) -> usize {
    let shapes: Vec<Vec<(u8, usize)>> = bars
        .iter()
        .map(|bar| {
            let mut shape: Vec<(u8, usize)> = bar.iter().map(|h| (h.key, h.step)).collect();
            shape.sort_unstable();
            shape.dedup();
            shape
        })
        .collect();
    let n = shapes.len();
    (1..n)
        .filter(|unit| n.is_multiple_of(*unit))
        .find(|&unit| (unit..n).all(|i| shapes[i] == shapes[i % unit]))
        .unwrap_or(n)
}

impl Project {
    /// Adds the bars as one new pattern after the existing ones, after collapsing exact repeats
    /// (see [`repeat_unit`]) and keeping at most [`MAX_BARS`] bars. Each drum plays on the first
    /// row that already plays it; a drum the project lacks gets a new row, while there is room.
    /// Tempo, swing, kit and song do not change.
    ///
    /// Refused with [`Refused::TooManyPatterns`] when the project is full, and with
    /// [`Refused::OutOfRange`] when there are no bars. Nothing changes then.
    pub fn add_imported_pattern(
        &mut self,
        bars: &[Vec<BarHit>],
    ) -> Result<(Imported, Vec<Change>), Refused> {
        if bars.is_empty() {
            return Err(Refused::OutOfRange);
        }
        let unit = repeat_unit(bars);
        let kept = unit.min(MAX_BARS);
        let bars_used = &bars[..kept];

        let (index, mut changes) = self.add_pattern()?;
        for _ in 1..kept {
            changes.extend(self.add_bar(index)?);
        }

        let mut rows_added = 0;
        let mut drums_left_out = 0;
        // The row for each drum. New rows go at the bottom, kick first and percussion last.
        let mut keys: Vec<u8> = bars_used.iter().flatten().map(|h| h.key).collect();
        keys.sort_unstable_by_key(|&k| (family_order(k), k));
        keys.dedup();
        let mut row_of: Vec<(u8, Option<usize>)> = Vec::new();
        for key in keys {
            let row = match self.rows.iter().position(|r| r.key == key) {
                Some(row) => Some(row),
                None if self.rows.len() < MAX_ROWS => {
                    changes.extend(self.add_row(key)?);
                    rows_added += 1;
                    Some(self.rows.len() - 1)
                }
                None => {
                    drums_left_out += 1;
                    None
                }
            };
            row_of.push((key, row));
        }

        for (bar, hits) in bars_used.iter().enumerate() {
            for hit in hits {
                let row = row_of
                    .iter()
                    .find(|(key, _)| *key == hit.key)
                    .and_then(|(_, row)| *row);
                let step = bar * STEPS_PER_BAR + hit.step;
                // Two hits of one drum on one step keep the louder.
                if let Some(row) = row
                    && hit.step < STEPS_PER_BAR
                    && hit.velocity > self.patterns[index].velocity(row, step)
                {
                    changes.extend(self.set_step(index, row, step, hit.velocity.min(127))?);
                }
            }
        }

        Ok((
            Imported {
                index,
                bars: kept,
                bars_given: bars.len(),
                bars_left_out: unit - kept,
                rows_added,
                drums_left_out,
            },
            changes,
        ))
    }

    /// A new project whose only pattern, A, holds `bars`, read as
    /// [`Project::add_imported_pattern`] reads them, at `bpm`. Everything else is as in
    /// [`Project::new`]. Refused with [`Refused::OutOfRange`] when there are no bars.
    pub fn from_imported(bars: &[Vec<BarHit>], bpm: f32) -> Result<(Project, Imported), Refused> {
        let mut project = Project::new();
        let (imported, _) = project.add_imported_pattern(bars)?;
        // The new pattern takes the place and the engine slot of the empty pattern A.
        let mut pattern = project.patterns.remove(imported.index);
        pattern.slot = project.patterns[0].slot;
        project.patterns[0] = pattern;
        project.tempo = bpm;
        Ok((
            project,
            Imported {
                index: 0,
                ..imported
            },
        ))
    }
}
