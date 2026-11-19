use super::*;
use crate::{DEFAULT_ROWS, from_json, to_json, to_midi};

#[test]
fn every_preset_builds_a_sensible_project() {
    assert!(presets().len() >= 6);
    for preset in presets() {
        let p = preset_project(preset.name).unwrap_or_else(|| panic!("{}", preset.name));
        assert!(!preset.description.is_empty());

        // The default rows, so every other feature works.
        let rows: Vec<(&str, u8)> = p.rows().iter().map(|r| (r.name.as_str(), r.key)).collect();
        assert_eq!(rows, DEFAULT_ROWS, "{}", preset.name);

        // A song that only names patterns the project has.
        assert!(!p.song().is_empty(), "{}", preset.name);
        for &slot in p.song() {
            assert!(p.index_of_slot(slot).is_some(), "{}", preset.name);
        }

        // At least one note in every pattern, and none hidden.
        for pattern in p.patterns() {
            assert!(
                (0..pattern.bars()).any(|bar| pattern.bar_has_notes(bar)),
                "{} {}",
                preset.name,
                pattern.name()
            );
            assert!(!pattern.has_hidden_notes(), "{}", preset.name);
        }

        assert!((40.0..=240.0).contains(&p.tempo), "{}", preset.name);
        assert!(!to_midi(&p).is_empty());
    }
}

#[test]
fn every_preset_round_trips_through_the_file() {
    for preset in presets() {
        let p = preset_project(preset.name).unwrap();
        let text = to_json(&p);
        assert_eq!(from_json(&text).unwrap(), p, "{}", preset.name);
    }
}

#[test]
fn preset_names_are_unique_and_found_ignoring_case() {
    for (i, a) in presets().iter().enumerate() {
        for b in &presets()[i + 1..] {
            assert!(!a.name.eq_ignore_ascii_case(b.name));
        }
    }
    assert_eq!(preset_project("rock"), preset_project("Rock"));
    assert!(preset_project("Polka").is_none());
}

#[test]
fn step_strings_read_as_velocities() {
    assert_eq!(
        velocities(".xXo|x"),
        vec![
            0,
            DEFAULT_VELOCITY,
            ACCENT_VELOCITY,
            SOFT_VELOCITY,
            DEFAULT_VELOCITY
        ]
    );
}

#[test]
fn the_rock_preset_has_the_expected_beat() {
    let p = preset_project("Rock").unwrap();
    assert_eq!(p.tempo, 110.0);
    assert_eq!(p.song(), &[0, 0, 0, 1]);
    let a = &p.patterns()[0];
    let kicks: Vec<usize> = (0..16).filter(|&s| a.velocity(KICK, s) > 0).collect();
    let snares: Vec<usize> = (0..16).filter(|&s| a.velocity(SNARE, s) > 0).collect();
    assert_eq!(kicks, vec![0, 8, 10]);
    assert_eq!(snares, vec![4, 12]);
    assert_eq!(a.velocity(HAT, 0), ACCENT_VELOCITY);
    assert_eq!(
        p.step_levels(),
        StepLevels::LoudAndSoft,
        "the hi-hat accents are reachable"
    );
}

#[test]
fn swing_presets_keep_their_swing_and_a_two_bar_pattern_has_two_bars() {
    assert_eq!(preset_project("Hip-hop").unwrap().swing(), 0.16);
    assert_eq!(preset_project("Reggae").unwrap().swing(), 0.33);
    assert_eq!(preset_project("Rock").unwrap().swing(), 0.0);
    let bossa = preset_project("Bossa nova").unwrap();
    assert_eq!(bossa.patterns()[0].bars(), 2);
    assert_eq!(bossa.patterns()[0].velocity(SNARE, 16 + 4), SOFT_VELOCITY);
}
