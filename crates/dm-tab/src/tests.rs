use dm_project::{
    ACCENT_VELOCITY, DEFAULT_ROWS, DEFAULT_VELOCITY, MAX_BARS, MAX_PATTERNS, MAX_ROWS,
    SOFT_VELOCITY,
};

use super::*;

fn hit(key: u8, step: usize, velocity: u8) -> Hit {
    Hit {
        key,
        step,
        velocity,
    }
}

/// The steps of one drum in a bar, in order.
fn steps(bar: &[Hit], key: u8) -> Vec<usize> {
    let mut s: Vec<usize> = bar
        .iter()
        .filter(|h| h.key == key)
        .map(|h| h.step)
        .collect();
    s.sort_unstable();
    s
}

const ROCK: &str = "\
HH|x-x-x-x-x-x-x-x-|x-x-x-x-x-x-x-x-|
S |----o-------o---|----o-------o---|
B |o-------o-o-----|o-------o-------|  x4
";

#[test]
fn a_two_bar_tab_with_a_repeat_mark_plays_four_times() {
    let tab = parse(ROCK).unwrap();
    assert_eq!(tab.bars.len(), 8);
    assert_eq!(steps(&tab.bars[0], 42), vec![0, 2, 4, 6, 8, 10, 12, 14]);
    assert_eq!(steps(&tab.bars[0], 38), vec![4, 12]);
    assert_eq!(steps(&tab.bars[0], 36), vec![0, 8, 10]);
    assert_eq!(steps(&tab.bars[1], 36), vec![0, 8]);
    assert_eq!(tab.bars[2], tab.bars[0]);
    assert_eq!(tab.bars[7], tab.bars[1]);
    assert!(tab.bars[0].iter().all(|h| h.velocity == DEFAULT_VELOCITY));
    assert_eq!(tab.tempo, None);
    assert!(tab.dropped_labels.is_empty());
    assert_eq!(tab.merged, 0);
}

#[test]
fn a_repeat_mark_on_the_line_after_a_system_counts_too() {
    let tab = parse("HH|x-x-x-x-x-x-x-x-|\nS |----x-------x---|\n   x3\n").unwrap();
    assert_eq!(tab.bars.len(), 3);
}

#[test]
fn repeats_stop_at_sixteen_plays() {
    let tab = parse("S |----x-------x---| x99\n").unwrap();
    assert_eq!(tab.bars.len(), 16);
}

#[test]
fn o_on_a_hi_hat_row_is_the_open_hat() {
    let tab = parse("HH|x-x-x-x-x-x-x-o-|\nB |o-------o-------|\n").unwrap();
    assert_eq!(steps(&tab.bars[0], 42), vec![0, 2, 4, 6, 8, 10, 12]);
    assert_eq!(steps(&tab.bars[0], 46), vec![14]);
    assert_eq!(steps(&tab.bars[0], 36), vec![0, 8]);
}

#[test]
fn ghosts_are_soft_and_capitals_are_accents() {
    // Parentheses take a column each, as tabs write them.
    let tab = parse("S |----X--g----X(x)|\n").unwrap();
    let mut bar = tab.bars[0].clone();
    bar.sort_by_key(|h| h.step);
    assert_eq!(
        bar,
        vec![
            hit(38, 4, ACCENT_VELOCITY),
            hit(38, 7, SOFT_VELOCITY),
            hit(38, 12, ACCENT_VELOCITY),
            hit(38, 14, SOFT_VELOCITY),
        ]
    );
}

#[test]
fn eight_column_bars_are_eighths() {
    let tab = parse("HH|x-x-x-x-|x-x-x-x-|\nS |--x---x-|--x---x-|\n").unwrap();
    assert_eq!(tab.bars.len(), 2);
    assert_eq!(steps(&tab.bars[0], 42), vec![0, 4, 8, 12]);
    assert_eq!(steps(&tab.bars[0], 38), vec![4, 12]);
}

#[test]
fn thirty_two_column_bars_are_halved_and_close_hits_merge() {
    let hats = format!("HH|xx{}x-|", "-".repeat(28));
    let snare = format!("S |{}x{}|", "-".repeat(8), "-".repeat(23));
    let tab = parse(&format!("{hats}\n{snare}\n")).unwrap();
    assert_eq!(steps(&tab.bars[0], 42), vec![0, 15]);
    assert_eq!(steps(&tab.bars[0], 38), vec![4]);
    assert_eq!(tab.merged, 1);
}

#[test]
fn merging_keeps_the_louder_hit() {
    let hats = format!("HH|xX{}|", "-".repeat(30));
    let tab = parse(&format!("{hats}\n")).unwrap();
    assert_eq!(tab.bars[0], vec![hit(42, 0, ACCENT_VELOCITY)]);
    assert_eq!(tab.merged, 1);
}

#[test]
fn a_spacer_column_without_a_leading_bar_line_is_trimmed() {
    let tab = parse("H -x---x---x---x---|\nS -----x-------x---|\n").unwrap();
    assert_eq!(tab.bars.len(), 1);
    assert_eq!(steps(&tab.bars[0], 42), vec![0, 4, 8, 12]);
    assert_eq!(steps(&tab.bars[0], 38), vec![4, 12]);
}

#[test]
fn a_bar_one_column_short_is_padded() {
    let tab = parse("HH|x-x-x-x-x-x-x-x|\nS |----x-------x--|\n").unwrap();
    assert_eq!(steps(&tab.bars[0], 42), vec![0, 2, 4, 6, 8, 10, 12, 14]);
}

#[test]
fn a_percent_bar_repeats_the_bar_before() {
    let tab = parse("HH|x-x-x-x-x-x-x-x-|       %        |\n").unwrap();
    assert_eq!(tab.bars.len(), 2);
    assert_eq!(tab.bars[1], tab.bars[0]);
}

#[test]
fn a_row_without_bar_lines_is_cut_into_bars_of_sixteen() {
    let tab = parse("HH x-x-x-x-x-x-x-x-x---x---x---x---\n").unwrap();
    assert_eq!(tab.bars.len(), 2);
    assert_eq!(steps(&tab.bars[1], 42), vec![0, 4, 8, 12]);
}

#[test]
fn the_tabs_own_legend_wins_over_the_table() {
    let text = "T = Tambourine\n\nT |x---x---x---x---|\nS |----x-------x---|\n";
    let tab = parse(text).unwrap();
    assert_eq!(steps(&tab.bars[0], 54), vec![0, 4, 8, 12]);
    assert!(steps(&tab.bars[0], 50).is_empty());
}

#[test]
fn a_lowercase_symbol_in_the_legend_does_not_redefine_a_row() {
    let text = "b = ride bell\n\nB |x-------x-------|\n";
    let tab = parse(text).unwrap();
    assert_eq!(steps(&tab.bars[0], 36), vec![0, 8]);
    assert!(steps(&tab.bars[0], 53).is_empty());
}

#[test]
fn an_unknown_row_with_few_hits_is_dropped() {
    let text = "HH|x-x-x-x-x-x-x-x-|\nS |----x-------x---|\nZz|x---------------|\n";
    let tab = parse(text).unwrap();
    assert_eq!(tab.dropped_labels, vec!["Zz".to_owned()]);
    assert_eq!(tab.bars[0].len(), 10);
}

#[test]
fn an_unknown_row_with_many_hits_is_refused() {
    let text = "HH|x-x-x-x-x-x-x-x-|\nS |----x-------x---|\nZz|x-------x-------|\n";
    assert_eq!(
        parse(text),
        Err(Refusal::UnknownDrums(vec!["Zz".to_owned()]))
    );
}

#[test]
fn triplet_width_bars_are_refused() {
    let text = "HH|x-x-x-x-x-x-|x-x-x-x-x-x-|\nS |---x-----x--|---x-----x--|\n";
    assert_eq!(parse(text), Err(Refusal::OddBars(vec![12])));
}

#[test]
fn rows_with_different_bar_counts_are_refused() {
    let text = "HH|x-x-x-x-x-x-x-x-|x-x-x-x-x-x-x-x-|\nS |----x-------x---|\n";
    assert_eq!(parse(text), Err(Refusal::Misaligned));
}

#[test]
fn prose_lyrics_guitar_tab_and_count_lines_are_skipped() {
    let text = "\
Back in Black, my version. Tempo: 94

Guitar:
e|-----------------|
B|-----------------|
G|-----5-----7-----|
D|---7-----7-------|
A|-5---------------|
E|-----------------|

Back in black, I hit the sack
Verse: play it twice

  1 e + a 2 e + a 3 e + a 4 e + a
HH|x-x-x-x-x-x-x-x-|
S |----o-------o---|
B |o-------o-------|

Thanks for reading!
";
    let tab = parse(text).unwrap();
    assert_eq!(tab.bars.len(), 1);
    assert_eq!(steps(&tab.bars[0], 36), vec![0, 8]);
    assert_eq!(steps(&tab.bars[0], 38), vec![4, 12]);
    assert_eq!(tab.bars[0].len(), 12);
    assert_eq!(tab.tempo, Some(94.0));
    assert!(tab.dropped_labels.is_empty());
}

#[test]
fn a_guitar_tab_alone_is_no_tab() {
    let text = "e|---3---|\nB|---0---|\nG|---0---|\nD|---0---|\nA|---2---|\nE|---3---|\n";
    assert_eq!(parse(text), Err(Refusal::NoTab));
}

#[test]
fn plain_prose_is_no_tab() {
    let text = "Hello there.\nThis is a note: nothing to play here.\nSee you - bye.\n";
    assert_eq!(parse(text), Err(Refusal::NoTab));
    assert_eq!(parse(""), Err(Refusal::NoTab));
}

#[test]
fn a_tempo_in_bpm_is_read() {
    let tab = parse("120 bpm\nS |----x-------x---|\n").unwrap();
    assert_eq!(tab.tempo, Some(120.0));
}

#[test]
fn adding_reuses_the_rows_the_project_has() {
    let mut project = Project::new();
    let tab = parse(ROCK).unwrap();
    let (added, changes) = add_to_project(&mut project, &tab).unwrap();
    assert_eq!(
        added,
        Added {
            index: 1,
            // The two bars played four times collapse to the two bars.
            bars: 2,
            bars_left_out: 0,
            rows_added: 0,
            drums_left_out: 0,
        }
    );
    assert!(!changes.is_empty());
    assert_eq!(project.rows().len(), DEFAULT_ROWS.len());
    assert_eq!(project.patterns().len(), 2);
    let pattern = &project.patterns()[1];
    assert_eq!(pattern.bars(), 2);
    // Kick is row 0, snare row 1, closed hat row 2.
    assert_eq!(pattern.velocity(0, 0), DEFAULT_VELOCITY);
    assert_eq!(pattern.velocity(0, 10), DEFAULT_VELOCITY);
    assert_eq!(pattern.velocity(1, 4), DEFAULT_VELOCITY);
    assert_eq!(pattern.velocity(2, 2), DEFAULT_VELOCITY);
    assert_eq!(pattern.velocity(1, 16 + 12), DEFAULT_VELOCITY);
    assert_eq!(pattern.velocity(0, 16 + 10), 0);
    // The first pattern and the song are untouched.
    assert!(!project.patterns()[0].bar_has_notes(0));
    assert_eq!(project.song(), &[0]);
}

#[test]
fn adding_adds_rows_for_missing_drums() {
    let mut project = Project::new();
    let tab = Tab {
        bars: vec![vec![hit(56, 0, 100), hit(54, 4, 100), hit(38, 4, 100)]],
        ..Default::default()
    };
    let (added, changes) = add_to_project(&mut project, &tab).unwrap();
    assert_eq!(added.rows_added, 2);
    assert_eq!(added.drums_left_out, 0);
    let rows = project.rows();
    assert_eq!(rows.len(), DEFAULT_ROWS.len() + 2);
    assert_eq!(rows[DEFAULT_ROWS.len()].key, 54);
    assert_eq!(rows[DEFAULT_ROWS.len() + 1].key, 56);
    assert!(changes.contains(&Change::RowKey {
        row: DEFAULT_ROWS.len(),
        key: 54
    }));
    let pattern = &project.patterns()[added.index];
    assert_eq!(pattern.velocity(DEFAULT_ROWS.len(), 4), 100);
    assert_eq!(pattern.velocity(DEFAULT_ROWS.len() + 1, 0), 100);
    assert_eq!(pattern.velocity(1, 4), 100);
}

#[test]
fn adding_leaves_out_drums_when_the_rows_are_full() {
    let mut project = Project::new();
    while project.rows().len() < MAX_ROWS {
        project.add_row(36).unwrap();
    }
    let tab = Tab {
        bars: vec![vec![hit(54, 0, 100), hit(36, 0, 100)]],
        ..Default::default()
    };
    let (added, _) = add_to_project(&mut project, &tab).unwrap();
    assert_eq!(added.rows_added, 0);
    assert_eq!(added.drums_left_out, 1);
    assert_eq!(project.patterns()[added.index].velocity(0, 0), 100);
}

#[test]
fn adding_keeps_the_first_sixteen_bars() {
    let mut project = Project::new();
    let tab = Tab {
        bars: (0..20).map(|i| vec![hit(36, i % 16, 100)]).collect(),
        ..Default::default()
    };
    let (added, _) = add_to_project(&mut project, &tab).unwrap();
    assert_eq!(added.bars, MAX_BARS);
    assert_eq!(added.bars_left_out, 4);
    let pattern = &project.patterns()[added.index];
    assert_eq!(pattern.bars(), MAX_BARS);
    assert_eq!(pattern.velocity(0, 15 * 16 + 15), 100);
}

#[test]
fn adding_is_refused_when_every_pattern_is_used() {
    let mut project = Project::new();
    while project.patterns().len() < MAX_PATTERNS {
        project.add_pattern().unwrap();
    }
    let before = project.clone();
    let tab = Tab {
        bars: vec![vec![hit(54, 0, 100)]],
        ..Default::default()
    };
    assert_eq!(
        add_to_project(&mut project, &tab),
        Err(AddError::NoFreePattern)
    );
    assert_eq!(project, before);
}
