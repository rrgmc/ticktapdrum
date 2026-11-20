use crate::{
    BarHit, MAX_BARS, MAX_PATTERNS, MAX_ROWS, Project, Refused, STEPS_PER_BAR, repeat_unit,
};

fn hit(key: u8, step: usize) -> BarHit {
    BarHit {
        key,
        step,
        velocity: 100,
    }
}

/// A bar with a kick on 1, a snare on 2 and 4, and the given extra hits.
fn bar(extra: &[(u8, usize)]) -> Vec<BarHit> {
    let mut hits = vec![hit(36, 0), hit(38, 4), hit(38, 12)];
    hits.extend(extra.iter().map(|&(k, s)| hit(k, s)));
    hits
}

#[test]
fn repeat_unit_finds_the_shortest_repeating_unit() {
    let a = bar(&[]);
    let b = bar(&[(36, 8)]);
    assert_eq!(repeat_unit(&[a.clone(), a.clone(), a.clone()]), 1);
    assert_eq!(
        repeat_unit(&[a.clone(), b.clone(), a.clone(), b.clone()]),
        2
    );
    // A fill at the end breaks the repeat.
    assert_eq!(
        repeat_unit(&[a.clone(), a.clone(), a.clone(), b.clone()]),
        4
    );
    // Three bars cannot be a 2-bar unit repeated.
    assert_eq!(repeat_unit(&[a.clone(), b.clone(), a.clone()]), 3);
    assert_eq!(repeat_unit(&[]), 0);
}

#[test]
fn repeat_unit_ignores_velocity_and_order() {
    let a = bar(&[]);
    let mut louder = a.clone();
    louder.reverse();
    louder[0].velocity = 127;
    assert_eq!(repeat_unit(&[a, louder]), 1);
}

#[test]
fn import_adds_one_pattern_reusing_rows() {
    let mut p = Project::new();
    let rows = p.rows().len();
    let (added, changes) = p.add_imported_pattern(&[bar(&[(42, 2)])]).unwrap();
    assert_eq!(added.index, 1);
    assert_eq!(added.bars, 1);
    assert_eq!(added.rows_added, 0);
    assert_eq!(p.rows().len(), rows);
    assert!(!changes.is_empty());
    let pattern = &p.patterns()[1];
    let kick = p.rows().iter().position(|r| r.key == 36).unwrap();
    let snare = p.rows().iter().position(|r| r.key == 38).unwrap();
    assert_eq!(pattern.velocity(kick, 0), 100);
    assert_eq!(pattern.velocity(snare, 4), 100);
    assert_eq!(pattern.velocity(snare, 12), 100);
}

#[test]
fn import_adds_rows_for_new_drums() {
    let mut p = Project::new();
    let (added, _) = p.add_imported_pattern(&[bar(&[(56, 0)])]).unwrap();
    assert_eq!(added.rows_added, 1);
    let cowbell = p.rows().iter().position(|r| r.key == 56).unwrap();
    assert_eq!(p.patterns()[added.index].velocity(cowbell, 0), 100);
}

#[test]
fn import_collapses_repeats_and_keeps_at_most_16_bars() {
    let a = bar(&[]);
    let b = bar(&[(36, 8)]);
    let mut p = Project::new();
    let (added, _) = p
        .add_imported_pattern(&[a.clone(), b.clone(), a.clone(), b.clone()])
        .unwrap();
    assert_eq!(
        (added.bars, added.bars_given, added.bars_left_out),
        (2, 4, 0)
    );
    assert_eq!(p.patterns()[added.index].bars(), 2);

    // 20 different bars: 16 kept, 4 left out.
    let many: Vec<_> = (0..20)
        .map(|i| bar(&[(42, i % STEPS_PER_BAR), (46, i / 4)]))
        .collect();
    let (added, _) = p.add_imported_pattern(&many).unwrap();
    assert_eq!((added.bars, added.bars_left_out), (MAX_BARS, 4));
}

#[test]
fn import_is_refused_when_the_project_is_full() {
    let mut p = Project::new();
    while p.patterns().len() < MAX_PATTERNS {
        p.add_pattern().unwrap();
    }
    let before = p.clone();
    assert_eq!(
        p.add_imported_pattern(&[bar(&[])]),
        Err(Refused::TooManyPatterns)
    );
    assert_eq!(p, before);
    assert_eq!(p.add_imported_pattern(&[]), Err(Refused::OutOfRange));
}

#[test]
fn import_leaves_out_drums_without_room_for_a_row() {
    let mut p = Project::new();
    while p.rows().len() < MAX_ROWS {
        p.add_row(60).unwrap();
    }
    let (added, _) = p.add_imported_pattern(&[bar(&[(81, 3)])]).unwrap();
    assert_eq!(added.drums_left_out, 1);
    assert_eq!(p.rows().len(), MAX_ROWS);
}
