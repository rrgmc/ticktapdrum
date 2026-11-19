use super::*;
use crate::{DEFAULT_ROWS, DEFAULT_VELOCITY, STEPS_PER_BAR};

/// The data model example from the design, with an integer volume.
const DESIGN_EXAMPLE: &str = r#"{
  "format": 1,
  "tempo_bpm": 100,
  "swing": 0.0,
  "step_levels": "on_off",
  "soundfont": { "kind": "colombogmgs2-17.11" },
  "kit_program": 0,
  "rows": [ { "id": 1, "name": "Kick", "key": 36, "volume": 100, "mute": false, "solo": false } ],
  "patterns": [ { "id": "A", "bar_count": 1, "steps": { "1": [[0, 100], [8, 100], [16, 100]] } } ],
  "song": { "order": ["A"], "loop": true }
}"#;

fn round_trip(project: &Project) {
    let text = to_json(project);
    let loaded = from_json(&text).unwrap_or_else(|e| panic!("{e}\n{text}"));
    assert_eq!(&loaded, project, "{text}");
    // Saving the loaded project gives the same text.
    assert_eq!(to_json(&loaded), text);
}

/// The design example with one top-level field replaced by `value`.
fn example_with(field: &str, value: Value) -> String {
    let mut file: Value = serde_json::from_str(DESIGN_EXAMPLE).unwrap();
    file[field] = value;
    file.to_string()
}

fn load_err(text: &str) -> LoadError {
    from_json(text).expect_err("the file should be refused")
}

/// Checks that `text` is refused for `reason`, and that the English text names the reason.
fn assert_invalid(text: &str, reason: InvalidReason, english: &str) {
    let error = load_err(text);
    assert_eq!(error, LoadError::Invalid(reason));
    assert_eq!(
        error.to_string(),
        format!("This file is damaged or incomplete: {english}.")
    );
}

#[test]
fn a_new_project_round_trips() {
    round_trip(&Project::new());
}

#[test]
fn a_busy_project_round_trips() {
    let mut p = Project::new();
    p.tempo = 132.5;
    p.song_loop = false;
    p.set_kit(25);
    p.set_swing(0.33);
    p.set_step_levels(StepLevels::Soft);

    // Pattern A: three bars that play and notes in two hidden bars.
    for _ in 0..4 {
        p.add_bar(0).unwrap();
    }
    p.set_step(0, 0, 0, DEFAULT_VELOCITY).unwrap();
    p.set_step(0, 1, 20, 60).unwrap();
    p.set_step(0, 2, 3 * STEPS_PER_BAR + 1, 127).unwrap();
    p.set_step(0, 9, MAX_STEPS - 1, 1).unwrap();
    p.remove_bar(0).unwrap();
    p.remove_bar(0).unwrap();
    assert!(p.patterns()[0].has_hidden_notes());

    // B, a copy of A placed right after it, then C, then delete B so the letters have a gap.
    p.add_pattern().unwrap();
    p.copy_pattern(0).unwrap();
    p.set_step(1, 3, 5, 90).unwrap();
    p.add_pattern().unwrap();
    p.set_step(3, 4, 7, 40).unwrap();
    p.delete_pattern(2).unwrap();
    let letters: String = p.patterns().iter().map(|p| p.name()).collect();
    assert_eq!(letters, "ACD");

    // A song with repeats.
    for index in [0, 0, 1, 2, 1, 0] {
        p.song_append(index).unwrap();
    }

    // Rows renamed, moved, muted, soloed, removed, added and changed.
    p.rename_row(0, "Big kick").unwrap();
    p.move_row(0, 5).unwrap();
    p.toggle_mute(2).unwrap();
    p.toggle_solo(3).unwrap();
    p.set_volume(4, 37).unwrap();
    p.remove_row(8).unwrap();
    p.add_row(56).unwrap();
    p.set_row_key(1, 40).unwrap();
    p.set_step(2, p.rows().len() - 1, 15, 100).unwrap();

    round_trip(&p);
}

#[test]
fn a_project_with_every_row_and_pattern_round_trips() {
    let mut p = Project::new();
    while p.add_row(35).is_ok() {}
    while p.add_pattern().is_ok() {}
    assert_eq!(p.rows().len(), MAX_ROWS);
    assert_eq!(p.patterns().len(), MAX_PATTERNS);
    for index in 0..MAX_PATTERNS {
        p.set_step(index, index, index, 100).unwrap();
        p.set_step(index, MAX_ROWS - 1, MAX_STEPS - 1, 100).unwrap();
    }
    round_trip(&p);
}

#[test]
fn the_design_example_loads_with_its_hidden_step_kept() {
    let p = from_json(DESIGN_EXAMPLE).unwrap();
    assert_eq!(p.rows().len(), 1);
    assert_eq!(p.rows()[0].name, "Kick");
    assert_eq!(p.rows()[0].key, 36);
    assert_eq!(p.tempo, 100.0);
    assert_eq!(p.kit(), 0);
    assert!(p.song_loop);
    assert_eq!(p.song(), &[0]);
    let a = &p.patterns()[0];
    assert_eq!(a.bars(), 1);
    assert_eq!(a.velocity(0, 0), 100);
    assert_eq!(a.velocity(0, 8), 100);
    assert_eq!(a.velocity(0, 16), 100);
    assert!(a.has_hidden_notes());
    // And it is written back out.
    assert!(to_json(&p).contains("\"1\": [[0, 100], [8, 100], [16, 100]]"));
}

#[test]
fn text_that_is_not_json_is_refused() {
    let error = load_err("this is not json");
    assert!(matches!(error, LoadError::NotJson(_)), "{error:?}");
    assert!(
        error
            .to_string()
            .starts_with("This file is not a TickTapDrum project.")
    );
}

#[test]
fn a_newer_format_is_refused() {
    let error = load_err(&example_with("format", 2.into()));
    assert_eq!(error, LoadError::NewerFormat(2));
    assert_eq!(
        error.to_string(),
        "This file was made by a newer version of TickTapDrum (file format 2). \
         Update the app to open it."
    );
    // The format is checked before the layout, which a newer version may have changed.
    assert_eq!(
        load_err(r#"{"format": 7, "something": "else"}"#),
        LoadError::NewerFormat(7)
    );
}

#[test]
fn a_missing_or_zero_format_is_refused() {
    let mut file: Value = serde_json::from_str(DESIGN_EXAMPLE).unwrap();
    file.as_object_mut().unwrap().remove("format");
    assert_invalid(
        &file.to_string(),
        InvalidReason::FormatMissing,
        "the file format number is missing",
    );
    assert_invalid(
        &example_with("format", 0.into()),
        InvalidReason::FormatZero,
        "the file format number is 0",
    );
    assert_invalid(
        &example_with("format", "1".into()),
        InvalidReason::FormatNotWholeNumber,
        "the file format number is not a whole number",
    );
    assert_invalid(
        "[1, 2]",
        InvalidReason::NotAnObject,
        "it does not hold a project",
    );
}

#[test]
fn duplicate_pattern_letters_are_refused() {
    let patterns = serde_json::json!([
        { "id": "A", "bar_count": 1, "steps": {} },
        { "id": "A", "bar_count": 2, "steps": {} }
    ]);
    assert_invalid(
        &example_with("patterns", patterns),
        InvalidReason::DuplicatePattern("A".into()),
        "two patterns are named A",
    );
}

#[test]
fn a_pattern_name_that_is_not_a_letter_is_refused() {
    let patterns = serde_json::json!([{ "id": "AB", "bar_count": 1 }]);
    assert_invalid(
        &example_with("patterns", patterns),
        InvalidReason::BadPatternName("AB".into()),
        "\"AB\" is not a pattern name, which must be one letter from A to Z",
    );
}

#[test]
fn a_song_naming_a_missing_pattern_is_refused() {
    let song = serde_json::json!({ "order": ["A", "B"], "loop": true });
    assert_invalid(
        &example_with("song", song),
        InvalidReason::SongPatternMissing("B".into()),
        "the song plays pattern \"B\", which is not in the file",
    );
}

#[test]
fn a_song_that_is_too_long_is_refused() {
    let song = serde_json::json!({ "order": vec!["A"; MAX_SONG + 1], "loop": true });
    assert_invalid(
        &example_with("song", song),
        InvalidReason::SongTooLong {
            count: 257,
            max: 256,
        },
        "the song has 257 entries, and the most allowed is 256",
    );
}

#[test]
fn zero_or_too_many_rows_are_refused() {
    assert_invalid(
        &example_with("rows", serde_json::json!([])),
        InvalidReason::NoRows,
        "the project has no drum rows",
    );
    let rows: Vec<Value> = (1..=33)
        .map(|id| serde_json::json!({ "id": id, "name": "Kick", "key": 36 }))
        .collect();
    assert_invalid(
        &example_with("rows", rows.into()),
        InvalidReason::TooManyRows { count: 33, max: 32 },
        "the project has 33 drum rows, and the most allowed is 32",
    );
}

#[test]
fn duplicate_row_ids_are_refused() {
    let rows = serde_json::json!([
        { "id": 1, "name": "Kick", "key": 36 },
        { "id": 1, "name": "Snare", "key": 38 }
    ]);
    assert_invalid(
        &example_with("rows", rows),
        InvalidReason::DuplicateRowId(1),
        "two drum rows have the id 1",
    );
}

#[test]
fn zero_or_too_many_patterns_are_refused() {
    assert_invalid(
        &example_with("patterns", serde_json::json!([])),
        InvalidReason::NoPatterns,
        "the project has no patterns",
    );
    let patterns: Vec<Value> = (0..27)
        .map(|i| serde_json::json!({ "id": slot_name(i).to_string() }))
        .collect();
    assert_invalid(
        &example_with("patterns", patterns.into()),
        InvalidReason::TooManyPatterns { count: 27, max: 26 },
        "the project has 27 patterns, and the most allowed is 26",
    );
}

#[test]
fn a_file_missing_a_required_part_is_refused() {
    let mut file: Value = serde_json::from_str(DESIGN_EXAMPLE).unwrap();
    file.as_object_mut().unwrap().remove("rows");
    let error = load_err(&file.to_string());
    assert!(
        matches!(&error, LoadError::Invalid(InvalidReason::Layout(detail)) if detail.contains("rows")),
        "{error:?}"
    );
}

#[test]
fn numbers_out_of_range_are_clamped() {
    let text = r#"{
      "format": 1,
      "tempo_bpm": 10,
      "rows": [
        { "id": 1, "name": "Kick", "key": 36, "volume": 150 },
        { "id": 2, "name": "Snare", "key": 38, "volume": -5 }
      ],
      "patterns": [
        { "id": "A", "bar_count": 0,
          "steps": { "1": [[0, 200], [1, 0], [2, -3], [256, 100], [-1, 100], [255, 50]],
                     "99": [[0, 100]], "x": [[0, 100]] } },
        { "id": "B", "bar_count": 40 }
      ],
      "song": { "order": ["B", "A", "B"] }
    }"#;
    let p = from_json(text).unwrap();
    assert_eq!(p.tempo, 40.0);
    assert_eq!(p.rows()[0].volume, 100);
    assert_eq!(p.rows()[1].volume, 0);
    let a = &p.patterns()[0];
    assert_eq!(a.bars(), 1);
    assert_eq!(a.velocity(0, 0), 127);
    assert_eq!(a.velocity(0, 1), 0);
    assert_eq!(a.velocity(0, 2), 0);
    assert_eq!(a.velocity(0, 255), 50);
    assert_eq!(a.velocity(1, 0), 0);
    assert_eq!(p.patterns()[1].bars(), MAX_BARS);
    assert_eq!(p.song(), &[1, 0, 1]);

    let fast = from_json(&example_with("tempo_bpm", 500.into())).unwrap();
    assert_eq!(fast.tempo, 240.0);
}

#[test]
fn unknown_fields_and_missing_optional_ones_are_accepted() {
    let text = r#"{
      "format": 1,
      "future_setting": { "anything": [1, 2, 3] },
      "swing": 0.4,
      "step_levels": "loud_and_soft",
      "soundfont": { "kind": "file", "name": "mine.sf2", "extra": 1 },
      "rows": [ { "id": 1, "key": 38, "colour": "red" } ],
      "patterns": [ { "id": "C", "steps": { "1": [[3, 90]] }, "notes": "hello" } ],
      "song": { "order": ["C"], "tempo_changes": [] }
    }"#;
    let p = from_json(text).unwrap();
    assert_eq!(p.tempo, DEFAULT_TEMPO);
    assert_eq!(p.kit(), DEFAULT_KIT);
    assert_eq!(p.rows()[0].name, "Snare");
    assert_eq!(p.rows()[0].volume, DEFAULT_VOLUME);
    assert!(!p.rows()[0].mute && !p.rows()[0].solo);
    assert_eq!(p.patterns()[0].name(), 'C');
    assert_eq!(p.patterns()[0].bars(), 1);
    assert_eq!(p.patterns()[0].velocity(0, 3), 90);
    assert!(p.song_loop);
    assert_eq!(p.swing(), 0.4);
    assert_eq!(p.step_levels(), StepLevels::LoudAndSoft);

    // Without a soundfont or a song at all.
    let mut file: Value = serde_json::from_str(DESIGN_EXAMPLE).unwrap();
    file.as_object_mut().unwrap().remove("soundfont");
    file.as_object_mut().unwrap().remove("song");
    let p = from_json(&file.to_string()).unwrap();
    assert!(p.song().is_empty());
}

#[test]
fn swing_and_step_levels_load_with_defaults_and_clamping() {
    // The design example holds straight timing, and taps that turn steps on and off.
    let p = from_json(DESIGN_EXAMPLE).unwrap();
    assert_eq!(p.swing(), 0.0);
    assert_eq!(p.step_levels(), StepLevels::OnOff);

    // Missing fields read as the defaults.
    let mut file: Value = serde_json::from_str(DESIGN_EXAMPLE).unwrap();
    file.as_object_mut().unwrap().remove("swing");
    file.as_object_mut().unwrap().remove("step_levels");
    let p = from_json(&file.to_string()).unwrap();
    assert_eq!(p.swing(), 0.0);
    assert_eq!(p.step_levels(), StepLevels::OnOff);

    // Each choice reads back, and one this version does not know reads as on and off.
    for (text, levels) in [
        ("on_off", StepLevels::OnOff),
        ("soft", StepLevels::Soft),
        ("loud_and_soft", StepLevels::LoudAndSoft),
        ("louder", StepLevels::OnOff),
    ] {
        let p = from_json(&example_with("step_levels", text.into())).unwrap();
        assert_eq!(p.step_levels(), levels, "{text}");
    }

    // Out-of-range swing is clamped.
    let p = from_json(&example_with("swing", 0.9.into())).unwrap();
    assert_eq!(p.swing(), MAX_SWING);
    let p = from_json(&example_with("swing", (-1).into())).unwrap();
    assert_eq!(p.swing(), 0.0);
}

#[test]
fn the_written_file_has_a_fixed_layout() {
    let mut p = Project::new();
    while p.rows().len() > 2 {
        p.remove_row(2).unwrap();
    }
    assert_eq!(p.rows().len(), 2);
    assert_eq!(p.rows()[1].name, DEFAULT_ROWS[1].0);
    p.toggle_mute(1).unwrap();
    p.set_step(0, 0, 0, 100).unwrap();
    p.set_step(0, 0, 8, 100).unwrap();
    p.set_step(0, 1, 4, 80).unwrap();
    p.set_step(0, 1, 17, 100).unwrap();
    p.add_pattern().unwrap();
    p.add_bar(1).unwrap();
    p.song_append(1).unwrap();
    p.song_append(0).unwrap();
    p.set_swing(0.25);
    p.set_step_levels(StepLevels::LoudAndSoft);

    let expected = r#"{
  "format": 1,
  "tempo_bpm": 100.0,
  "swing": 0.25,
  "step_levels": "loud_and_soft",
  "soundfont": {
    "kind": "colombogmgs2-17.11"
  },
  "kit_program": 0,
  "rows": [
    {
      "id": 1,
      "name": "Kick",
      "key": 36,
      "volume": 100,
      "mute": false,
      "solo": false
    },
    {
      "id": 2,
      "name": "Snare",
      "key": 38,
      "volume": 100,
      "mute": true,
      "solo": false
    }
  ],
  "patterns": [
    {
      "id": "A",
      "bar_count": 1,
      "steps": {
        "1": [[0, 100], [8, 100]],
        "2": [[4, 80], [17, 100]]
      }
    },
    {
      "id": "B",
      "bar_count": 2,
      "steps": {}
    }
  ],
  "song": {
    "order": ["A", "B", "A"],
    "loop": true
  }
}
"#;
    assert_eq!(to_json(&p), expected);
}
