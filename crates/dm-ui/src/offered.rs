//! Drums played on the MIDI keyboard that have no row in the project. The grid shows them as
//! inactive rows under the others, and a click on one adds it as a row. See "Milestone 14: MIDI
//! in" in `docs/DESIGN.md`.
//!
//! They belong to this session, not to the project, and are never saved. While a take records,
//! a hit on one of them is held in it as a pending note, which becomes a real step only when the
//! row is added. Pending notes never play: only the keyboard sounded them.

use std::collections::BTreeMap;

use dm_project::{Change, Project, Refused};

/// The most drums offered at once.
pub(crate) const MAX_OFFERED: usize = 3;

/// One drum on offer, with the notes recorded on it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct OfferedRow {
    /// The General MIDI drum key.
    pub(crate) key: u8,
    /// The level of each pending note, by pattern slot and step. Slots rather than pattern
    /// positions, so a note stays with its pattern if the patterns move.
    notes: BTreeMap<(usize, usize), u8>,
}

impl OfferedRow {
    /// The level of the pending note on `step` of the pattern in `slot`; 0 for none.
    pub(crate) fn velocity(&self, slot: usize, step: usize) -> u8 {
        self.notes.get(&(slot, step)).copied().unwrap_or(0)
    }
}

/// The drums on offer, oldest first.
#[derive(Debug, Default)]
pub(crate) struct Offered {
    rows: Vec<OfferedRow>,
}

impl Offered {
    /// The drums on offer, oldest first, as the grid shows them from the top.
    pub(crate) fn rows(&self) -> &[OfferedRow] {
        &self.rows
    }

    /// The keys on offer, oldest first.
    pub(crate) fn keys(&self) -> Vec<u8> {
        self.rows.iter().map(|r| r.key).collect()
    }

    pub(crate) fn len(&self) -> usize {
        self.rows.len()
    }

    /// Offers `key`, a drum with no row. A key already on offer keeps its place. With
    /// [`MAX_OFFERED`] drums on offer, the oldest is pushed out with its pending notes.
    pub(crate) fn offer(&mut self, key: u8) {
        if self.rows.iter().any(|r| r.key == key) {
            return;
        }
        if self.rows.len() >= MAX_OFFERED {
            self.rows.remove(0);
        }
        self.rows.push(OfferedRow {
            key,
            notes: BTreeMap::new(),
        });
    }

    /// Holds a note of `key` at `level` on `step` of the pattern in `slot`, offering the key
    /// first if needed. A step already held keeps the louder of the two levels.
    pub(crate) fn hold(&mut self, key: u8, slot: usize, step: usize, level: u8) {
        self.offer(key);
        if let Some(row) = self.rows.iter_mut().find(|r| r.key == key) {
            let held = row.notes.entry((slot, step)).or_insert(0);
            *held = (*held).max(level);
        }
    }

    /// Drops every pending note and keeps the drums on offer. For Undo, Redo, and another
    /// project, which the notes were not recorded against.
    pub(crate) fn drop_notes(&mut self) {
        for row in &mut self.rows {
            row.notes.clear();
        }
    }

    /// Takes `key` off offer, with its pending notes.
    pub(crate) fn remove(&mut self, key: u8) -> Option<OfferedRow> {
        let at = self.rows.iter().position(|r| r.key == key)?;
        Some(self.rows.remove(at))
    }

    /// Takes off offer the drums that now have a row in `project`, for example one added from
    /// the Sounds dialog or found in a project just opened.
    pub(crate) fn forget_rows_of(&mut self, project: &Project) {
        self.rows
            .retain(|o| !project.rows().iter().any(|r| r.key == o.key));
    }

    /// Adds the drum `key` on offer as a row at the bottom of `project`, with its pending
    /// notes at the levels played, and takes it off offer. The step levels move up to reach
    /// those notes, as a take's do. Returns the changes for the engine.
    ///
    /// With [`dm_project::MAX_ROWS`] rows already, this is refused and the drum stays on offer.
    pub(crate) fn add_to(
        &mut self,
        project: &mut Project,
        key: u8,
    ) -> Result<Vec<Change>, Refused> {
        if project.rows().iter().any(|r| r.key == key) {
            self.remove(key);
            return Ok(Vec::new());
        }
        let mut changes = project.add_row(key)?;
        let row = project.rows().len() - 1;
        let notes = self.remove(key).map(|o| o.notes).unwrap_or_default();
        for ((slot, step), level) in notes {
            let Some(index) = project.patterns().iter().position(|p| p.slot() == slot) else {
                continue;
            };
            if step >= project.patterns()[index].length() {
                continue;
            }
            changes.extend(project.set_step(index, row, step, level)?);
            crate::record::raise_levels(project, level);
        }
        Ok(changes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::{Group, History};
    use dm_project::{ACCENT_VELOCITY, DEFAULT_VELOCITY, MAX_ROWS, SOFT_VELOCITY, StepLevels};

    const COWBELL: u8 = 56;
    const TAMBOURINE: u8 = 54;
    const CLAVES: u8 = 75;
    const AGOGO: u8 = 67;

    #[test]
    fn at_most_three_are_offered_and_the_oldest_goes() {
        let mut o = Offered::default();
        o.offer(COWBELL);
        o.offer(TAMBOURINE);
        o.offer(CLAVES);
        assert_eq!(o.keys(), vec![COWBELL, TAMBOURINE, CLAVES]);
        o.offer(AGOGO);
        assert_eq!(o.keys(), vec![TAMBOURINE, CLAVES, AGOGO]);
    }

    #[test]
    fn a_key_already_offered_keeps_its_place() {
        let mut o = Offered::default();
        o.offer(COWBELL);
        o.offer(TAMBOURINE);
        o.offer(COWBELL);
        o.hold(COWBELL, 0, 4, DEFAULT_VELOCITY);
        assert_eq!(o.keys(), vec![COWBELL, TAMBOURINE]);
    }

    #[test]
    fn a_held_step_keeps_the_louder_level() {
        let mut o = Offered::default();
        o.hold(COWBELL, 0, 4, ACCENT_VELOCITY);
        o.hold(COWBELL, 0, 4, SOFT_VELOCITY);
        assert_eq!(o.rows()[0].velocity(0, 4), ACCENT_VELOCITY);
        o.hold(COWBELL, 0, 6, SOFT_VELOCITY);
        o.hold(COWBELL, 0, 6, DEFAULT_VELOCITY);
        assert_eq!(o.rows()[0].velocity(0, 6), DEFAULT_VELOCITY);
        // Another pattern's step is its own.
        assert_eq!(o.rows()[0].velocity(1, 4), 0);
    }

    #[test]
    fn a_pushed_out_key_takes_its_notes() {
        let mut o = Offered::default();
        o.hold(COWBELL, 0, 4, DEFAULT_VELOCITY);
        o.offer(TAMBOURINE);
        o.offer(CLAVES);
        o.offer(AGOGO);
        // Offered again, it comes back at the bottom with nothing held.
        o.offer(COWBELL);
        assert_eq!(o.keys(), vec![CLAVES, AGOGO, COWBELL]);
        assert_eq!(o.rows()[2].velocity(0, 4), 0);
    }

    #[test]
    fn dropping_notes_keeps_the_keys() {
        let mut o = Offered::default();
        o.hold(COWBELL, 0, 4, DEFAULT_VELOCITY);
        o.drop_notes();
        assert_eq!(o.keys(), vec![COWBELL]);
        assert_eq!(o.rows()[0].velocity(0, 4), 0);
    }

    #[test]
    fn keys_that_now_have_a_row_are_forgotten() {
        let mut p = Project::new();
        let mut o = Offered::default();
        o.offer(COWBELL);
        o.offer(TAMBOURINE);
        p.add_row(COWBELL).unwrap();
        o.forget_rows_of(&p);
        assert_eq!(o.keys(), vec![TAMBOURINE]);
    }

    #[test]
    fn adding_copies_the_notes_and_raises_the_levels() {
        let mut p = Project::new();
        p.add_pattern().unwrap();
        let slot0 = p.patterns()[0].slot();
        let slot1 = p.patterns()[1].slot();
        let rows = p.rows().len();
        let mut o = Offered::default();
        o.offer(TAMBOURINE);
        o.hold(COWBELL, slot0, 2, SOFT_VELOCITY);
        o.hold(COWBELL, slot1, 8, DEFAULT_VELOCITY);
        assert_eq!(p.step_levels(), StepLevels::OnOff);

        let changes = o.add_to(&mut p, COWBELL).unwrap();
        assert_eq!(p.rows().len(), rows + 1);
        assert_eq!(p.rows()[rows].key, COWBELL);
        assert_eq!(p.patterns()[0].velocity(rows, 2), SOFT_VELOCITY);
        assert_eq!(p.patterns()[1].velocity(rows, 8), DEFAULT_VELOCITY);
        assert_eq!(p.step_levels(), StepLevels::Soft);
        assert!(changes.contains(&Change::RowKey {
            row: rows,
            key: COWBELL
        }));
        assert!(changes.contains(&Change::Step {
            slot: slot1,
            row: rows,
            step: 8,
            velocity: DEFAULT_VELOCITY
        }));
        // It leaves the offer; the others stay.
        assert_eq!(o.keys(), vec![TAMBOURINE]);
    }

    #[test]
    fn adding_is_one_undo_step() {
        let mut p = Project::new();
        let start = p.clone();
        let mut history = History::default();
        let mut o = Offered::default();
        o.hold(COWBELL, p.patterns()[0].slot(), 0, ACCENT_VELOCITY);
        o.hold(COWBELL, p.patterns()[0].slot(), 4, SOFT_VELOCITY);
        let before = p.clone();
        o.add_to(&mut p, COWBELL).unwrap();
        history.record(&before, Group::Single, 0.0);
        assert_eq!(p.step_levels(), StepLevels::LoudAndSoft);
        assert!(history.undo(&mut p));
        assert_eq!(p, start);
        assert!(!history.can_undo());
    }

    #[test]
    fn adding_to_full_rows_is_refused_and_keeps_the_offer() {
        let mut p = Project::new();
        let mut key = 60;
        while p.rows().len() < MAX_ROWS {
            p.add_row(key).unwrap();
            key += 1;
        }
        let before = p.clone();
        let mut o = Offered::default();
        o.hold(COWBELL, 0, 4, DEFAULT_VELOCITY);
        assert_eq!(o.add_to(&mut p, COWBELL), Err(Refused::TooManyRows));
        assert_eq!(p, before);
        assert_eq!(o.keys(), vec![COWBELL]);
        assert_eq!(o.rows()[0].velocity(0, 4), DEFAULT_VELOCITY);
    }
}
