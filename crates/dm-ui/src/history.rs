//! Undo and redo, as whole-project snapshots.
//!
//! A project is small (a few kilobytes for a typical one), so keeping copies is simpler and safer
//! than recording each change's inverse. Edits that belong to one gesture, such as a drag that
//! paints several steps or one slide of a volume slider, share a group and undo together. So do
//! the notes of one MIDI recording take.

use dm_project::Project;

/// How many steps back undo can go.
const LIMIT: usize = 100;
/// Edits of the same group closer together than this, in seconds, undo as one.
const MERGE_WINDOW: f64 = 1.5;

/// What kind of edit a snapshot was taken for.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Group {
    /// Steps painted in one press of the finger or mouse button, numbered per press.
    Paint(u64),
    /// One row's volume while its slider moves.
    Volume(usize),
    /// The tempo while it is being changed.
    Tempo,
    /// The swing amount while its slider moves.
    Swing,
    /// The notes one MIDI recording take added, numbered per take. A take undoes as one step
    /// however long it lasts, so it merges whatever the time between edits.
    Recording(u64),
    /// Anything else: always its own undo step.
    Single,
}

#[derive(Default)]
pub(crate) struct History {
    undo: Vec<Project>,
    redo: Vec<Project>,
    /// The group of the newest undo step and when it was last extended.
    open: Option<(Group, f64)>,
}

impl History {
    /// Records `before`, the project as it was before an edit that changed it.
    pub(crate) fn record(&mut self, before: &Project, group: Group, now: f64) {
        if group != Group::Single
            && let Some((open, at)) = self.open
            && open == group
            && (matches!(group, Group::Recording(_)) || now - at < MERGE_WINDOW)
        {
            self.open = Some((group, now));
            return;
        }
        self.undo.push(before.clone());
        if self.undo.len() > LIMIT {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.open = Some((group, now));
    }

    /// Steps back one edit. Returns false when there is nothing to undo.
    pub(crate) fn undo(&mut self, current: &mut Project) -> bool {
        let Some(previous) = self.undo.pop() else {
            return false;
        };
        self.redo.push(std::mem::replace(current, previous));
        self.open = None;
        true
    }

    /// Steps forward one undone edit. Returns false when there is nothing to redo.
    pub(crate) fn redo(&mut self, current: &mut Project) -> bool {
        let Some(next) = self.redo.pop() else {
            return false;
        };
        self.undo.push(std::mem::replace(current, next));
        self.open = None;
        true
    }

    pub(crate) fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub(crate) fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Forgets everything, for a project that was just opened or started.
    pub(crate) fn clear(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dm_project::DEFAULT_VELOCITY;

    fn edit(project: &mut Project, step: usize) -> Project {
        let before = project.clone();
        project.set_step(0, 0, step, DEFAULT_VELOCITY).unwrap();
        before
    }

    #[test]
    fn undo_and_redo_walk_back_and_forth() {
        let mut history = History::default();
        let mut p = Project::new();
        let before = edit(&mut p, 0);
        history.record(&before, Group::Single, 0.0);
        let before = edit(&mut p, 1);
        history.record(&before, Group::Single, 0.1);
        assert!(history.undo(&mut p));
        assert_eq!(p.patterns()[0].velocity(0, 1), 0);
        assert_eq!(p.patterns()[0].velocity(0, 0), DEFAULT_VELOCITY);
        assert!(history.redo(&mut p));
        assert_eq!(p.patterns()[0].velocity(0, 1), DEFAULT_VELOCITY);
        assert!(!history.redo(&mut p));
    }

    #[test]
    fn one_paint_gesture_undoes_as_one() {
        let mut history = History::default();
        let mut p = Project::new();
        for step in 0..4 {
            let before = edit(&mut p, step);
            history.record(&before, Group::Paint(7), step as f64 * 0.1);
        }
        assert!(history.undo(&mut p));
        assert_eq!(p, Project::new());
        assert!(!history.can_undo());
    }

    #[test]
    fn a_new_gesture_or_a_pause_starts_a_new_step() {
        let mut history = History::default();
        let mut p = Project::new();
        let before = edit(&mut p, 0);
        history.record(&before, Group::Paint(1), 0.0);
        let before = edit(&mut p, 1);
        history.record(&before, Group::Paint(2), 0.1);
        let before = edit(&mut p, 2);
        history.record(&before, Group::Paint(2), 5.0);
        assert!(history.undo(&mut p));
        assert!(history.undo(&mut p));
        assert!(history.undo(&mut p));
        assert!(!history.can_undo());
    }

    #[test]
    fn a_new_edit_clears_redo() {
        let mut history = History::default();
        let mut p = Project::new();
        let before = edit(&mut p, 0);
        history.record(&before, Group::Single, 0.0);
        history.undo(&mut p);
        let before = edit(&mut p, 3);
        history.record(&before, Group::Single, 1.0);
        assert!(!history.can_redo());
    }

    #[test]
    fn history_is_capped() {
        let mut history = History::default();
        let mut p = Project::new();
        for i in 0..(LIMIT + 10) {
            let before = edit(&mut p, i % 16);
            history.record(&before, Group::Single, i as f64);
        }
        let mut undone = 0;
        while history.undo(&mut p) {
            undone += 1;
        }
        assert_eq!(undone, LIMIT);
    }
}
