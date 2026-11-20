//! Recording from a MIDI input: each drum hit heard while the pattern loops becomes a step.
//!
//! The engine plays each hit and reports where in the beat the player heard it (see
//! `dm_audio::HeardHit`). The functions here turn one such hit into a step edit on the
//! [`Project`], or into a pending note on a drum with no row (see [`Offered`]), without an
//! engine, so they can be tested alone. The caller sends the changes on to the engine and keeps
//! one undo step per take. See "Milestone 14: MIDI in" in
//! `docs/DESIGN.md`.

use crate::offered::Offered;
use dm_project::{Change, DEFAULT_VELOCITY, Project, Refused, SOFT_VELOCITY, StepLevels};

/// A hit below this velocity makes a soft step. To be tuned by ear on a real keyboard.
pub(crate) const SOFT_BELOW: u8 = 64;

/// The step level a hit at `velocity` records: soft or normal. Recording never makes a loud
/// step: hard hits on the PSR-SX720 seldom passed 100, so loud stays a tap on the grid.
pub(crate) fn level(velocity: u8) -> u8 {
    if velocity < SOFT_BELOW {
        SOFT_VELOCITY
    } else {
        DEFAULT_VELOCITY
    }
}

/// The step nearest to `position` on the grid swung by `swing`, for a pattern of `length` steps.
/// `position` is in straight steps from the pattern's start, as the engine reports it. A hit
/// just before the loop's start lands on step 0.
pub(crate) fn nearest_step(position: f64, swing: f32, length: usize) -> usize {
    let length = length.max(1);
    let position = if position.is_finite() {
        position.clamp(0.0, length as f64)
    } else {
        0.0
    };
    let (step, _) = dm_groove::snap(position, f64::from(swing));
    step % length
}

/// The fewest step levels that reach a step of `velocity`.
fn levels_for(velocity: u8) -> StepLevels {
    if velocity > DEFAULT_VELOCITY {
        StepLevels::LoudAndSoft
    } else if velocity < DEFAULT_VELOCITY {
        StepLevels::Soft
    } else {
        StepLevels::OnOff
    }
}

/// How many levels a choice reaches, to compare two choices.
fn rank(levels: StepLevels) -> u8 {
    match levels {
        StepLevels::OnOff => 0,
        StepLevels::Soft => 1,
        StepLevels::LoudAndSoft => 2,
    }
}

/// Moves the step levels up to reach a step of `velocity`, as a beat to start from picks the
/// fewest levels that reach its notes. They never move down.
pub(crate) fn raise_levels(project: &mut Project, velocity: u8) {
    let needed = levels_for(velocity);
    if rank(needed) > rank(project.step_levels()) {
        project.set_step_levels(needed);
    }
}

/// Records one hit of `key` at `velocity` into the pattern at `index`, at `position` in straight
/// steps. Returns the changes for the engine; none when nothing changed.
///
/// - The hit plays on the first row with its key. A key with no row never changes the project:
///   the drum is offered in `offered`, and the hit waits there as a pending note.
/// - The step keeps the louder of what it had and what the hit gives. Recording never removes or
///   softens a step.
/// - The step levels move up to reach the level just recorded (see [`raise_levels`]).
pub(crate) fn record_hit(
    project: &mut Project,
    offered: &mut Offered,
    index: usize,
    key: u8,
    velocity: u8,
    position: f64,
) -> Result<Vec<Change>, Refused> {
    let pattern = project.pattern(index).ok_or(Refused::OutOfRange)?;
    let (slot, length) = (pattern.slot(), pattern.length());
    let step = nearest_step(position, project.swing(), length);
    let new = level(velocity);
    let Some(row) = project.rows().iter().position(|r| r.key == key) else {
        offered.hold(key, slot, step, new);
        return Ok(Vec::new());
    };
    let mut changes = Vec::new();
    let old = project.patterns()[index].velocity(row, step);
    if new > old {
        changes.extend(project.set_step(index, row, step, new)?);
        raise_levels(project, new);
    }
    Ok(changes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::{Group, History};
    use dm_project::ACCENT_VELOCITY;

    const KICK: u8 = 36;
    const SNARE: u8 = 38;
    /// Not among the default rows.
    const COWBELL: u8 = 56;

    /// Records a hit, with nothing on offer before or after.
    fn hit(
        p: &mut Project,
        index: usize,
        key: u8,
        velocity: u8,
        position: f64,
    ) -> Result<Vec<Change>, Refused> {
        record_hit(p, &mut Offered::default(), index, key, velocity, position)
    }

    #[test]
    fn velocity_sets_the_level() {
        assert_eq!(level(1), SOFT_VELOCITY);
        assert_eq!(level(SOFT_BELOW - 1), SOFT_VELOCITY);
        assert_eq!(level(SOFT_BELOW), DEFAULT_VELOCITY);
        assert_eq!(level(127), DEFAULT_VELOCITY);
    }

    #[test]
    fn a_hit_lands_on_the_nearest_straight_step() {
        assert_eq!(nearest_step(0.0, 0.0, 16), 0);
        assert_eq!(nearest_step(0.4, 0.0, 16), 0);
        assert_eq!(nearest_step(0.6, 0.0, 16), 1);
        assert_eq!(nearest_step(4.49, 0.0, 16), 4);
        assert_eq!(nearest_step(7.51, 0.0, 16), 8);
        assert_eq!(nearest_step(14.9, 0.0, 32), 15);
    }

    #[test]
    fn swing_moves_the_odd_steps() {
        // With a third of a step of swing, step 1 sounds at 1.33.
        let swing = 0.33;
        assert_eq!(nearest_step(1.2, swing, 16), 1);
        assert_eq!(nearest_step(1.6, swing, 16), 1);
        assert_eq!(nearest_step(0.6, swing, 16), 0);
        // Halfway between 1.33 and 2 is 1.67: past it is step 2.
        assert_eq!(nearest_step(1.7, swing, 16), 2);
        // Without swing the same hits land elsewhere.
        assert_eq!(nearest_step(0.6, 0.0, 16), 1);
        assert_eq!(nearest_step(1.6, 0.0, 16), 2);
    }

    #[test]
    fn a_hit_just_before_the_loop_starts_lands_on_the_first_step() {
        assert_eq!(nearest_step(15.7, 0.0, 16), 0);
        assert_eq!(nearest_step(31.9, 0.0, 32), 0);
        assert_eq!(nearest_step(16.0, 0.0, 16), 0);
        // With swing the last step sits later, so a late hit may still belong to it.
        assert_eq!(nearest_step(15.7, 0.5, 16), 15);
        assert_eq!(nearest_step(15.9, 0.5, 16), 0);
        // Nonsense positions land somewhere in the pattern rather than panic.
        assert_eq!(nearest_step(-0.3, 0.0, 16), 0);
        assert_eq!(nearest_step(f64::NAN, 0.0, 16), 0);
        assert_eq!(nearest_step(99.0, 0.0, 16), 0);
    }

    #[test]
    fn a_hit_sets_a_step_and_reports_it() {
        let mut p = Project::new();
        let changes = hit(&mut p, 0, SNARE, 90, 4.1).unwrap();
        assert_eq!(p.patterns()[0].velocity(1, 4), DEFAULT_VELOCITY);
        assert_eq!(
            changes,
            vec![Change::Step {
                slot: p.patterns()[0].slot(),
                row: 1,
                step: 4,
                velocity: DEFAULT_VELOCITY
            }]
        );
    }

    #[test]
    fn a_step_keeps_the_louder_level() {
        let mut p = Project::new();
        hit(&mut p, 0, KICK, 120, 0.0).unwrap();
        assert_eq!(p.patterns()[0].velocity(0, 0), DEFAULT_VELOCITY);
        let changes = hit(&mut p, 0, KICK, 20, 0.0).unwrap();
        assert!(changes.is_empty());
        assert_eq!(p.patterns()[0].velocity(0, 0), DEFAULT_VELOCITY);
        // A loud step set by tapping stays loud.
        p.set_step(0, 0, 4, ACCENT_VELOCITY).unwrap();
        assert!(hit(&mut p, 0, KICK, 127, 4.0).unwrap().is_empty());
        assert_eq!(p.patterns()[0].velocity(0, 4), ACCENT_VELOCITY);

        hit(&mut p, 0, KICK, 20, 8.0).unwrap();
        assert_eq!(p.patterns()[0].velocity(0, 8), SOFT_VELOCITY);
        hit(&mut p, 0, KICK, 90, 8.0).unwrap();
        assert_eq!(p.patterns()[0].velocity(0, 8), DEFAULT_VELOCITY);
    }

    #[test]
    fn a_key_with_no_row_is_offered_not_added() {
        let mut p = Project::new();
        let before = p.clone();
        let mut o = Offered::default();
        let changes = record_hit(&mut p, &mut o, 0, COWBELL, 90, 2.0).unwrap();
        assert!(changes.is_empty());
        assert_eq!(p, before);
        assert_eq!(o.keys(), vec![COWBELL]);
        let slot = p.patterns()[0].slot();
        assert_eq!(o.rows()[0].velocity(slot, 2), DEFAULT_VELOCITY);

        // The same snapping and level as a real row, and the louder hit wins.
        record_hit(&mut p, &mut o, 0, COWBELL, 20, 5.9).unwrap();
        record_hit(&mut p, &mut o, 0, COWBELL, 127, 6.2).unwrap();
        record_hit(&mut p, &mut o, 0, COWBELL, 20, 2.0).unwrap();
        assert_eq!(o.rows()[0].velocity(slot, 6), DEFAULT_VELOCITY);
        assert_eq!(o.rows()[0].velocity(slot, 2), DEFAULT_VELOCITY);
        assert_eq!(p, before);
        // The step levels wait until the row is added.
        assert_eq!(p.step_levels(), StepLevels::OnOff);
    }

    #[test]
    fn a_key_with_a_row_is_not_offered() {
        let mut p = Project::new();
        let mut o = Offered::default();
        record_hit(&mut p, &mut o, 0, KICK, 90, 0.0).unwrap();
        assert_eq!(o.len(), 0);
    }

    #[test]
    fn step_levels_move_up_to_fit_and_never_down() {
        let mut p = Project::new();
        assert_eq!(p.step_levels(), StepLevels::OnOff);
        hit(&mut p, 0, KICK, 90, 0.0).unwrap();
        assert_eq!(p.step_levels(), StepLevels::OnOff);
        hit(&mut p, 0, KICK, 20, 2.0).unwrap();
        assert_eq!(p.step_levels(), StepLevels::Soft);
        // A hard hit is normal, and never asks for loud.
        hit(&mut p, 0, KICK, 127, 4.0).unwrap();
        assert_eq!(p.step_levels(), StepLevels::Soft);

        // Levels already chosen are kept.
        let mut p = Project::new();
        p.set_step_levels(StepLevels::LoudAndSoft);
        hit(&mut p, 0, KICK, 20, 0.0).unwrap();
        assert_eq!(p.step_levels(), StepLevels::LoudAndSoft);
    }

    #[test]
    fn a_hit_into_a_missing_pattern_is_refused() {
        let mut p = Project::new();
        assert!(hit(&mut p, 5, KICK, 90, 0.0).is_err());
    }

    #[test]
    fn one_take_undoes_as_one_step() {
        let mut history = History::default();
        let mut p = Project::new();
        let start = p.clone();
        // Hits far apart in time, each recorded as the app records them.
        for (i, (key, step)) in [(KICK, 0.0), (SNARE, 4.0), (COWBELL, 6.0), (KICK, 8.0)]
            .into_iter()
            .enumerate()
        {
            let before = p.clone();
            hit(&mut p, 0, key, 100, step).unwrap();
            history.record(&before, Group::Recording(1), i as f64 * 10.0);
        }
        assert!(history.undo(&mut p));
        assert_eq!(p, start);
        assert!(!history.can_undo());

        // A second take is a step of its own.
        let mut history = History::default();
        let mut p = Project::new();
        for take in [1, 2] {
            let before = p.clone();
            hit(&mut p, 0, KICK, 100, take as f64 * 4.0).unwrap();
            history.record(&before, Group::Recording(take), 0.0);
        }
        assert!(history.undo(&mut p));
        assert!(history.undo(&mut p));
        assert!(!history.can_undo());
    }
}
