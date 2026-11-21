use super::*;

fn on(project: &mut Project, index: usize, row: usize, step: usize) {
    project
        .set_step(index, row, step, DEFAULT_VELOCITY)
        .unwrap();
}

#[test]
fn a_new_project_has_pattern_a_of_one_bar_in_the_song() {
    let p = Project::new();
    assert_eq!(p.rows().len(), DEFAULT_ROWS.len());
    assert_eq!(p.patterns().len(), 1);
    assert_eq!(p.patterns()[0].name(), 'A');
    assert_eq!(p.patterns()[0].bars(), 1);
    assert_eq!(p.song(), &[0]);
}

#[test]
fn removing_a_bar_hides_its_notes_and_adding_it_back_shows_them() {
    let mut p = Project::new();
    p.add_bar(0).unwrap();
    on(&mut p, 0, 0, 20);
    let changes = p.remove_bar(0).unwrap();
    assert_eq!(changes, vec![Change::Length { slot: 0, steps: 16 }]);
    assert!(p.patterns()[0].has_hidden_notes());
    assert_eq!(p.patterns()[0].velocity(0, 20), DEFAULT_VELOCITY);
    p.add_bar(0).unwrap();
    assert!(!p.patterns()[0].has_hidden_notes());
    assert_eq!(p.patterns()[0].velocity(0, 20), DEFAULT_VELOCITY);
}

#[test]
fn the_last_bar_cannot_be_removed_and_bars_stop_at_the_maximum() {
    let mut p = Project::new();
    assert_eq!(p.remove_bar(0), Err(Refused::LastBar));
    for _ in 1..MAX_BARS {
        p.add_bar(0).unwrap();
    }
    assert_eq!(p.add_bar(0), Err(Refused::TooManyBars));
}

#[test]
fn clearing_hidden_bars_removes_only_hidden_notes() {
    let mut p = Project::new();
    p.add_bar(0).unwrap();
    on(&mut p, 0, 0, 0);
    on(&mut p, 0, 1, 17);
    p.remove_bar(0).unwrap();
    let changes = p.clear_hidden_bars(0).unwrap();
    assert_eq!(
        changes,
        vec![Change::Step {
            slot: 0,
            row: 1,
            step: 17,
            velocity: 0
        }]
    );
    assert_eq!(p.patterns()[0].velocity(0, 0), DEFAULT_VELOCITY);
    assert!(!p.patterns()[0].has_hidden_notes());
}

#[test]
fn copying_a_bar_inserts_it_after_and_moves_later_bars_without_loss() {
    let mut p = Project::new();
    p.add_bar(0).unwrap();
    on(&mut p, 0, 0, 2); // bar 0
    on(&mut p, 0, 1, 18); // bar 1
    on(&mut p, 0, 2, 35); // hidden bar 2
    p.copy_bar(0, 0).unwrap();
    let pat = &p.patterns()[0];
    assert_eq!(pat.bars(), 3);
    assert_eq!(pat.velocity(0, 2), DEFAULT_VELOCITY);
    assert_eq!(pat.velocity(0, 18), DEFAULT_VELOCITY, "the copy");
    assert_eq!(
        pat.velocity(1, 34),
        DEFAULT_VELOCITY,
        "old bar 1 moved to bar 2"
    );
    assert_eq!(
        pat.velocity(2, 51),
        DEFAULT_VELOCITY,
        "hidden bar moved to bar 3"
    );
    assert_eq!(pat.velocity(1, 18), 0);
    assert!(pat.has_hidden_notes());
}

#[test]
fn deleting_a_bar_removes_its_notes_and_moves_later_bars_back() {
    let mut p = Project::new();
    p.add_bar(0).unwrap();
    p.add_bar(0).unwrap();
    on(&mut p, 0, 0, 2); // bar 0
    on(&mut p, 0, 1, 18); // bar 1
    on(&mut p, 0, 2, 35); // bar 2
    on(&mut p, 0, 3, MAX_STEPS - 1); // last possible bar, hidden
    p.delete_bar(0, 1).unwrap();
    let pat = &p.patterns()[0];
    assert_eq!(pat.bars(), 2);
    assert_eq!(pat.velocity(0, 2), DEFAULT_VELOCITY, "bar 0 untouched");
    assert_eq!(pat.velocity(1, 18), 0, "bar 1 gone");
    assert_eq!(
        pat.velocity(2, 19),
        DEFAULT_VELOCITY,
        "bar 2 moved to bar 1"
    );
    assert_eq!(pat.velocity(2, 35), 0);
    assert_eq!(
        pat.velocity(3, MAX_STEPS - 1 - STEPS_PER_BAR),
        DEFAULT_VELOCITY,
        "hidden bars move too"
    );
    assert_eq!(pat.velocity(3, MAX_STEPS - 1), 0);
}

#[test]
fn deleting_a_bar_is_refused_for_the_only_bar_or_one_out_of_range() {
    let mut p = Project::new();
    assert_eq!(p.delete_bar(0, 0), Err(Refused::LastBar));
    p.add_bar(0).unwrap();
    assert_eq!(p.delete_bar(0, 2), Err(Refused::OutOfRange));
    assert_eq!(p.patterns()[0].bars(), 2);
}

#[test]
fn copying_is_refused_when_the_last_possible_bar_holds_notes() {
    let mut p = Project::new();
    on(&mut p, 0, 0, MAX_STEPS - 1);
    assert_eq!(p.copy_bar(0, 0), Err(Refused::NoRoomToCopy));
    assert_eq!(p.patterns()[0].bars(), 1);
}

#[test]
fn copying_a_bar_reports_exactly_the_engine_changes() {
    let mut p = Project::new();
    on(&mut p, 0, 0, 4);
    let changes = p.copy_bar(0, 0).unwrap();
    assert_eq!(
        changes,
        vec![
            Change::Length { slot: 0, steps: 32 },
            Change::Step {
                slot: 0,
                row: 0,
                step: 20,
                velocity: DEFAULT_VELOCITY
            },
        ]
    );
}

#[test]
fn patterns_take_the_first_free_slot() {
    let mut p = Project::new();
    let (b, _) = p.add_pattern().unwrap();
    let (c, _) = p.add_pattern().unwrap();
    assert_eq!(p.pattern(b).unwrap().name(), 'B');
    assert_eq!(p.pattern(c).unwrap().name(), 'C');
    p.delete_pattern(b).unwrap();
    let (again, _) = p.add_pattern().unwrap();
    assert_eq!(p.pattern(again).unwrap().name(), 'B');
}

#[test]
fn a_copied_pattern_keeps_its_hidden_bars() {
    let mut p = Project::new();
    p.add_bar(0).unwrap();
    on(&mut p, 0, 3, 20);
    p.remove_bar(0).unwrap();
    let (copy, changes) = p.copy_pattern(0).unwrap();
    assert_eq!(copy, 1);
    assert!(p.pattern(copy).unwrap().has_hidden_notes());
    assert!(changes.contains(&Change::Step {
        slot: 1,
        row: 3,
        step: 20,
        velocity: DEFAULT_VELOCITY
    }));
}

#[test]
fn removing_a_song_entry_keeps_the_pattern() {
    let mut p = Project::new();
    let (b, _) = p.add_pattern().unwrap();
    p.song_append(b).unwrap();
    p.song_remove(1).unwrap();
    assert_eq!(p.song(), &[0]);
    assert_eq!(p.patterns().len(), 2);
}

#[test]
fn deleting_a_pattern_removes_it_from_the_song_and_clears_its_slot() {
    let mut p = Project::new();
    let (b, _) = p.add_pattern().unwrap();
    on(&mut p, b, 0, 0);
    p.song_append(b).unwrap();
    p.song_append(0).unwrap();
    p.song_append(b).unwrap();
    let changes = p.delete_pattern(b).unwrap();
    assert_eq!(p.song(), &[0, 0]);
    assert!(changes.contains(&Change::Step {
        slot: 1,
        row: 0,
        step: 0,
        velocity: 0
    }));
    assert!(changes.contains(&Change::Song(vec![0, 0])));
}

#[test]
fn the_last_pattern_cannot_be_deleted() {
    let mut p = Project::new();
    assert_eq!(p.delete_pattern(0), Err(Refused::LastPattern));
}

#[test]
fn song_entries_move() {
    let mut p = Project::new();
    let (b, _) = p.add_pattern().unwrap();
    p.song_append(b).unwrap();
    p.song_move(1, 0).unwrap();
    assert_eq!(p.song(), &[1, 0]);
}

#[test]
fn full_sync_describes_every_pattern_and_the_song() {
    let mut p = Project::new();
    on(&mut p, 0, 0, 0);
    let (b, _) = p.add_pattern().unwrap();
    p.add_bar(b).unwrap();
    let changes = p.full_sync();
    assert!(changes.contains(&Change::Length { slot: 0, steps: 16 }));
    assert!(changes.contains(&Change::Length { slot: 1, steps: 32 }));
    assert!(changes.contains(&Change::Step {
        slot: 0,
        row: 0,
        step: 0,
        velocity: DEFAULT_VELOCITY
    }));
    assert_eq!(changes.last(), Some(&Change::Song(vec![0])));
}

#[test]
fn mute_silences_one_row_and_solo_silences_the_others() {
    let mut p = Project::new();
    p.toggle_mute(1).unwrap();
    assert_eq!(p.gain(1), 0);
    assert_eq!(p.gain(0), DEFAULT_VOLUME);
    let changes = p.toggle_solo(0).unwrap();
    assert_eq!(changes.len(), p.rows().len());
    assert_eq!(p.gain(0), DEFAULT_VOLUME);
    assert_eq!(p.gain(2), 0);
    p.toggle_solo(1).unwrap();
    assert_eq!(p.gain(1), 0, "a muted row stays silent even when soloed");
}

#[test]
fn volume_is_capped_and_sent_after_mute() {
    let mut p = Project::new();
    p.set_volume(0, 250).unwrap();
    assert_eq!(p.rows()[0].volume, 100);
    p.toggle_mute(0).unwrap();
    let changes = p.set_volume(0, 40).unwrap();
    assert_eq!(changes, vec![Change::RowGain { row: 0, percent: 0 }]);
    assert_eq!(p.rows()[0].volume, 40);
}

#[test]
fn moving_a_row_moves_its_notes_in_every_pattern() {
    let mut p = Project::new();
    on(&mut p, 0, 0, 3);
    let (b, _) = p.add_pattern().unwrap();
    on(&mut p, b, 0, 5);
    let changes = p.move_row(0, 2).unwrap();
    assert_eq!(p.rows()[2].name, "Kick");
    assert_eq!(p.patterns()[0].velocity(2, 3), DEFAULT_VELOCITY);
    assert_eq!(p.patterns()[b].velocity(2, 5), DEFAULT_VELOCITY);
    assert_eq!(p.patterns()[0].velocity(0, 3), 0);
    assert!(changes.contains(&Change::ClearSlot { slot: 0 }));
    assert!(changes.contains(&Change::RowKey { row: 2, key: 36 }));
    assert!(changes.contains(&Change::Step {
        slot: 0,
        row: 2,
        step: 3,
        velocity: DEFAULT_VELOCITY
    }));
}

#[test]
fn removing_a_row_removes_its_notes_and_the_last_row_stays() {
    let mut p = Project::new();
    on(&mut p, 0, 1, 0);
    on(&mut p, 0, 2, 4);
    assert!(p.row_has_notes(1));
    p.remove_row(1).unwrap();
    assert_eq!(p.rows().len(), DEFAULT_ROWS.len() - 1);
    assert_eq!(
        p.patterns()[0].velocity(1, 4),
        DEFAULT_VELOCITY,
        "the row below moved up"
    );
    while p.rows().len() > 1 {
        p.remove_row(0).unwrap();
    }
    assert_eq!(p.remove_row(0), Err(Refused::LastRow));
}

#[test]
fn added_rows_are_named_after_their_drum_and_stop_at_the_maximum() {
    let mut p = Project::new();
    let changes = p.add_row(56).unwrap();
    assert_eq!(p.rows().last().unwrap().name, "Cowbell");
    assert!(changes.contains(&Change::RowKey {
        row: DEFAULT_ROWS.len(),
        key: 56
    }));
    assert_eq!(p.patterns()[0].rows(), DEFAULT_ROWS.len() + 1);
    while p.rows().len() < MAX_ROWS {
        p.add_row(75).unwrap();
    }
    assert_eq!(p.add_row(75), Err(Refused::TooManyRows));
}

#[test]
fn changing_a_rows_drum_keeps_its_notes() {
    let mut p = Project::new();
    on(&mut p, 0, 0, 0);
    assert_eq!(
        p.set_row_key(0, 35).unwrap(),
        vec![Change::RowKey { row: 0, key: 35 }]
    );
    assert_eq!(p.patterns()[0].velocity(0, 0), DEFAULT_VELOCITY);
}

#[test]
fn swing_is_clamped_and_sent_in_thousandths() {
    let mut p = Project::new();
    assert_eq!(p.swing(), 0.0);
    assert_eq!(p.set_swing(0.33), vec![Change::Swing(330)]);
    assert_eq!(p.swing(), 0.33);
    assert_eq!(p.set_swing(0.9), vec![Change::Swing(500)]);
    assert_eq!(p.swing(), MAX_SWING);
    assert_eq!(p.set_swing(-0.2), vec![Change::Swing(0)]);
    assert_eq!(p.set_swing(f32::NAN), vec![Change::Swing(0)]);
    assert_eq!(p.swing(), 0.0);
}

#[test]
fn full_sync_includes_the_swing() {
    let mut p = Project::new();
    p.set_swing(0.125);
    assert!(p.full_sync().contains(&Change::Swing(125)));
    assert!(Project::new().full_sync().contains(&Change::Swing(0)));
}

#[test]
fn step_levels_are_a_setting_that_starts_on_and_off_and_cycles() {
    let mut p = Project::new();
    assert_eq!(p.step_levels(), StepLevels::OnOff);
    p.set_step_levels(StepLevels::Soft);
    assert_eq!(p.step_levels(), StepLevels::Soft);
    assert_eq!(StepLevels::OnOff.next(), StepLevels::Soft);
    assert_eq!(StepLevels::Soft.next(), StepLevels::LoudAndSoft);
    assert_eq!(StepLevels::LoudAndSoft.next(), StepLevels::OnOff);
}

#[test]
fn on_and_off_taps_only_turn_steps_on_and_off() {
    let on_off = StepLevels::OnOff;
    assert_eq!(next_level(0, on_off), DEFAULT_VELOCITY);
    assert_eq!(next_level(DEFAULT_VELOCITY, on_off), 0);
    assert_eq!(next_level(SOFT_VELOCITY, on_off), 0);
    assert_eq!(next_level(ACCENT_VELOCITY, on_off), 0);
}

#[test]
fn soft_taps_cycle_off_on_soft() {
    let soft = StepLevels::Soft;
    assert_eq!(next_level(0, soft), DEFAULT_VELOCITY);
    assert_eq!(next_level(DEFAULT_VELOCITY, soft), SOFT_VELOCITY);
    assert_eq!(next_level(SOFT_VELOCITY, soft), 0);
    // A loud step, from before the choice changed, becomes soft; a quieter one turns off.
    assert_eq!(next_level(ACCENT_VELOCITY, soft), SOFT_VELOCITY);
    assert_eq!(next_level(30, soft), 0);
}

#[test]
fn loud_and_soft_taps_cycle_off_normal_loud_soft() {
    let full = StepLevels::LoudAndSoft;
    assert_eq!(next_level(0, full), DEFAULT_VELOCITY);
    assert_eq!(next_level(DEFAULT_VELOCITY, full), ACCENT_VELOCITY);
    assert_eq!(next_level(ACCENT_VELOCITY, full), SOFT_VELOCITY);
    assert_eq!(next_level(SOFT_VELOCITY, full), 0);
    // Other velocities, as from a file, move to the quietest level louder than them.
    assert_eq!(next_level(1, full), SOFT_VELOCITY);
    assert_eq!(next_level(59, full), SOFT_VELOCITY);
    assert_eq!(next_level(61, full), DEFAULT_VELOCITY);
    assert_eq!(next_level(99, full), DEFAULT_VELOCITY);
    assert_eq!(next_level(101, full), ACCENT_VELOCITY);
    assert_eq!(next_level(126, full), ACCENT_VELOCITY);
    assert_eq!(next_level(200, full), SOFT_VELOCITY);
}

#[test]
fn full_sync_includes_kit_rows_and_gains() {
    let mut p = Project::new();
    p.set_kit(25);
    let changes = p.full_sync();
    assert_eq!(changes[0], Change::Kit(25));
    assert!(changes.contains(&Change::RowKey { row: 0, key: 36 }));
    assert!(changes.contains(&Change::RowGain {
        row: 0,
        percent: DEFAULT_VOLUME
    }));
}

#[test]
fn every_common_drum_is_a_general_midi_drum_and_none_repeats() {
    for (i, key) in COMMON_DRUMS.iter().enumerate() {
        assert!(
            GM_DRUMS.iter().any(|(k, _)| k == key),
            "{key} is not in GM_DRUMS"
        );
        assert!(!COMMON_DRUMS[..i].contains(key), "{key} repeats");
    }
}

#[test]
fn drums_fall_into_the_families_their_names_suggest() {
    assert_eq!(drum_family(36), DrumFamily::Kick);
    assert_eq!(drum_family(39), DrumFamily::Snare);
    assert_eq!(drum_family(46), DrumFamily::HiHat);
    assert_eq!(drum_family(41), DrumFamily::Tom);
    assert_eq!(drum_family(53), DrumFamily::Cymbal);
    assert_eq!(drum_family(56), DrumFamily::Percussion);
    assert_eq!(drum_family(81), DrumFamily::Percussion);
}

fn letters(p: &Project) -> String {
    p.patterns().iter().map(Pattern::name).collect()
}

fn song_letters(p: &Project) -> String {
    p.song().iter().map(|&slot| p.slot_name(slot)).collect()
}

#[test]
fn renaming_to_a_free_letter_changes_only_the_name() {
    let mut p = Project::new();
    let (b, _) = p.add_pattern().unwrap();
    p.song_append(b).unwrap();
    on(&mut p, b, 0, 4);
    let slot = p.pattern(b).unwrap().slot();
    p.rename_pattern(b, 'Q').unwrap();
    assert_eq!(letters(&p), "AQ");
    assert_eq!(song_letters(&p), "AQ");
    assert_eq!(p.pattern(b).unwrap().slot(), slot);
    assert_eq!(p.pattern(b).unwrap().velocity(0, 4), DEFAULT_VELOCITY);
}

#[test]
fn renaming_to_a_letter_in_use_swaps_the_two() {
    let mut p = Project::new();
    let (b, _) = p.add_pattern().unwrap();
    p.add_pattern().unwrap();
    p.song_append(b).unwrap();
    p.rename_pattern(0, 'C').unwrap();
    assert_eq!(letters(&p), "CBA");
    // The song still plays the same patterns, now under their new letters.
    assert_eq!(song_letters(&p), "CB");
    p.rename_pattern(0, 'C').unwrap();
    assert_eq!(letters(&p), "CBA");
}

#[test]
fn a_new_pattern_takes_the_first_letter_no_pattern_has() {
    let mut p = Project::new();
    p.rename_pattern(0, 'B').unwrap();
    let (a, _) = p.add_pattern().unwrap();
    assert_eq!(p.pattern(a).unwrap().name(), 'A');
    let (c, _) = p.copy_pattern(0).unwrap();
    assert_eq!(p.pattern(c).unwrap().name(), 'C');
    assert_eq!(letters(&p), "BCA");
}

#[test]
fn renaming_refuses_what_is_not_a_capital_letter() {
    let mut p = Project::new();
    assert_eq!(p.rename_pattern(0, 'a'), Err(Refused::OutOfRange));
    assert_eq!(p.rename_pattern(0, '1'), Err(Refused::OutOfRange));
    assert_eq!(p.rename_pattern(1, 'B'), Err(Refused::OutOfRange));
    assert_eq!(letters(&p), "A");
}

#[test]
fn renamed_letters_survive_saving_and_loading() {
    let mut p = Project::new();
    let (b, _) = p.add_pattern().unwrap();
    on(&mut p, b, 1, 2);
    p.song_append(b).unwrap();
    p.rename_pattern(b, 'Z').unwrap();
    p.rename_pattern(0, 'M').unwrap();
    let loaded = from_json(&to_json(&p)).unwrap();
    assert_eq!(letters(&loaded), "MZ");
    assert_eq!(song_letters(&loaded), "MZ");
    assert_eq!(loaded.pattern(1).unwrap().velocity(1, 2), DEFAULT_VELOCITY);
    // Loading puts each pattern in its letter's slot.
    assert_eq!(loaded.pattern(1).unwrap().slot(), 25);
}
