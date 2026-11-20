//! Tests of the style import, with small hand-built styles.

use dm_project::{KeyboardKind, Section, StyleSections};

use super::*;

/// Channel 10 and channel 9, counted from 0.
const RHYTHM: u8 = 9;
const SUB_RHYTHM: u8 = 8;
const CONGA: u8 = 63;

fn marker(tick: u32, text: &str) -> Event {
    let mut bytes = vec![0xFF, 0x06, text.len() as u8];
    bytes.extend_from_slice(text.as_bytes());
    Event(tick, bytes)
}

fn bank(tick: u32, channel: u8, msb: u8) -> Event {
    Event(tick, vec![0xB0 | channel, 0, msb])
}

fn program(tick: u32, channel: u8, program: u8) -> Event {
    Event(tick, vec![0xC0 | channel, program])
}

/// The setup bar: tempo 100, the time signature, the `SFF1` marker, and a kit for each
/// `(channel, msb, program)`.
fn setup(numerator: u8, kits: &[(u8, u8, u8)]) -> Vec<Event> {
    let mut events = vec![
        tempo(0, 100.0),
        time_signature(0, numerator, 2),
        marker(0, "SFF1"),
    ];
    for &(channel, msb, number) in kits {
        events.push(bank(0, channel, msb));
        events.push(program(0, channel, number));
    }
    events
}

/// A style file: the setup bar, then each section from a bar line, as `(marker, bars, events)`
/// with ticks counted from the section's start.
fn style(setup: Vec<Event>, sections: Vec<(&str, u32, Vec<Event>)>) -> Vec<u8> {
    let mut events = setup;
    let mut start = BAR;
    for (name, bars, section) in sections {
        // The marker goes before the section's notes at the same tick, as the sort is stable.
        events.push(marker(start, name));
        events.extend(
            section
                .into_iter()
                .map(|Event(tick, bytes)| Event(start + tick, bytes)),
        );
        start += bars * BAR;
    }
    smf(events, start)
}

/// A rock beat on a channel, without the tempo and time signature, with `hat` as the hi-hat key.
fn beat(bars: u32, channel: u8, hat: u8) -> Vec<Event> {
    rock(bars, hat)
        .into_iter()
        .filter(|e| e.1[0] & 0xF0 == 0x90)
        .map(|Event(tick, mut bytes)| {
            bytes[0] = 0x90 | channel;
            Event(tick, bytes)
        })
        .collect()
}

fn all_hits(groove: &Groove) -> BTreeSet<(usize, u8, usize, u8)> {
    groove
        .bars
        .iter()
        .enumerate()
        .flat_map(|(bar, hits)| hits.iter().map(move |h| (bar, h.key, h.step, h.velocity)))
        .collect()
}

fn keys(groove: &Groove) -> BTreeSet<u8> {
    groove.bars.iter().flatten().map(|h| h.key).collect()
}

/// Every built-in preset, written as a Yamaha style by the app, imports with the same hits in
/// every section.
#[test]
fn presets_round_trip_through_a_style() {
    const MAINS: [&str; 4] = ["Main A", "Main B", "Main C", "Main D"];
    const FILLS: [&str; 4] = ["Fill In AA", "Fill In BB", "Fill In CC", "Fill In DD"];
    for preset in dm_project::presets() {
        let project = dm_project::preset_project(preset.name).unwrap();
        let sections = StyleSections::suggested(&project);
        let bytes = KeyboardKind::Yamaha
            .write(&project, &sections, preset.name)
            .unwrap();
        assert!(is_style(&bytes), "{}", preset.name);
        let import = import(&format!("{}.sty", preset.name), &bytes).unwrap();
        assert!(
            import.refused.is_empty(),
            "{}: {:?}",
            preset.name,
            import.refused
        );

        // What each section should hold, as the writer lays it out.
        let mut expected = Vec::new();
        for section in Section::ALL {
            let (marker, main) = match section {
                Section::Intro => ("Intro A", None),
                Section::Main(i) => (MAINS[i], None),
                Section::Fill(i) => match sections.mains[i] {
                    Some(main) => (FILLS[i], Some(main)),
                    None => continue,
                },
                Section::Break => ("Fill In BA", None),
                Section::Ending => ("Ending A", None),
            };
            let (slot, last_bar_only) = match (sections.get(section), main) {
                (Some(slot), _) => (slot, false),
                (None, Some(main)) => (main, true),
                (None, None) => continue,
            };
            let pattern = &project.patterns()[project.index_of_slot(slot).unwrap()];
            let first = if last_bar_only {
                pattern.length() - 16
            } else {
                0
            };
            let mut hits = BTreeSet::new();
            for (row, info) in project.rows().iter().enumerate() {
                for step in first..pattern.length() {
                    let v = pattern.velocity(row, step);
                    if v > 0 {
                        let at = step - first;
                        hits.insert((at / 16, info.key, at % 16, v));
                    }
                }
            }
            expected.push((format!("{} {marker}", preset.name), hits));
        }

        assert_eq!(import.grooves.len(), expected.len(), "{}", preset.name);
        let kit = style::YamahaKit::find(127, project.kit()).map(|k| k.name.to_owned());
        for (groove, (name, hits)) in import.grooves.iter().zip(&expected) {
            assert_eq!(&groove.name, name);
            assert_eq!(groove.folder, "");
            assert_eq!(&all_hits(groove), hits, "{name}");
            assert_eq!(groove.bpm, project.tempo.round(), "{name}");
            assert_eq!(groove.kit, kit, "{name}");
        }
    }
}

/// Percussion on channel 9 goes through its kit's map and joins the drums. A quarter of the
/// notes on conga keys would refuse a MIDI file, but not a style.
#[test]
fn percussion_on_channel_nine_is_mapped_and_accepted() {
    let mut notes = beat(1, RHYTHM, HAT);
    // PopPercussionKit's key 24 is a conga, General MIDI 63.
    for step in [0, 4, 8, 12] {
        notes.push(note(step * STEP, SUB_RHYTHM, 24, 90));
    }
    let bytes = style(
        setup(4, &[(RHYTHM, 127, 0), (SUB_RHYTHM, 126, 45)]),
        vec![("Main A", 1, notes)],
    );
    let import = import("Pop.sty", &bytes).unwrap();
    assert_eq!(import.refused, vec![]);
    let groove = &import.grooves[0];
    assert_eq!(groove.name, "Pop Main A");
    assert_eq!(groove.bpm, 100.0);
    assert_eq!(
        groove.kit.as_deref(),
        Some("StandardKit1 + PopPercussionKit")
    );
    assert_eq!(groove.similar_kit, Some(0));
    assert_eq!(keys(groove), BTreeSet::from([KICK, SNARE, HAT, CONGA]));
    assert!(groove.bars[0].contains(&BarHit {
        key: CONGA,
        step: 4,
        velocity: 90
    }));
}

/// In a style, the tambourine and cowbell keys count as drums, since the key map put those
/// sounds there. A MIDI file with the same share of them is still refused.
#[test]
fn tambourine_and_cowbell_count_in_a_style() {
    let mut notes = beat(1, RHYTHM, HAT);
    // A tambourine (54) and a cowbell (56) on every eighth note: far more than 10% of the notes.
    for step in (0..16).step_by(2) {
        notes.push(note(step * STEP, SUB_RHYTHM, 54, 80));
        notes.push(note(step * STEP, SUB_RHYTHM, 56, 80));
    }
    let bytes = style(setup(4, &[(RHYTHM, 127, 0)]), vec![("Main A", 1, notes)]);
    let import = import("Pop.sty", &bytes).unwrap();
    assert_eq!(import.refused, vec![]);
    assert!(keys(&import.grooves[0]).is_superset(&BTreeSet::from([54, 56])));

    let mut events = rock(1, HAT);
    for step in (0..16).step_by(2) {
        events.push(note(step * STEP, RHYTHM, 54, 80));
        events.push(note(step * STEP, RHYTHM, 56, 80));
    }
    assert_eq!(one(&smf(events, BAR)), Err(Refusal::OtherDrumProgram));
}

/// A tempo change inside a section, such as an Ending slowing down, is ignored: the groove takes
/// the setup bar's tempo.
#[test]
fn a_tempo_change_inside_a_section_is_ignored() {
    let mut notes = beat(2, RHYTHM, HAT);
    notes.push(tempo(BAR, 80.0));
    let bytes = style(setup(4, &[(RHYTHM, 127, 0)]), vec![("Ending A", 2, notes)]);
    let import = import("Pop.sty", &bytes).unwrap();
    assert_eq!(import.refused, vec![]);
    assert_eq!(import.grooves[0].bpm, 100.0);
}

/// The rule for styles does not loosen it for MIDI files: the same notes in a MIDI file are
/// still made for another drum program.
#[test]
fn percussion_keys_still_refuse_a_midi_file() {
    let mut events = rock(1, HAT);
    for step in [0, 4, 8, 12] {
        events.push(note(step * STEP, RHYTHM, CONGA, 90));
    }
    assert_eq!(one(&smf(events, BAR)), Err(Refusal::OtherDrumProgram));
}

/// A kit in the table maps its own keys and leaves out the keys it does not list.
#[test]
fn kit_table_maps_keys_and_leaves_some_out() {
    // PopDrumKit: key 13 is a closed hi-hat, key 23 is left out.
    let mut notes = beat(1, RHYTHM, 13);
    notes.push(note(2 * STEP, RHYTHM, 23, 70));
    let bytes = style(setup(4, &[(RHYTHM, 127, 73)]), vec![("Main A", 1, notes)]);
    let import = import("Pop.sty", &bytes).unwrap();
    let groove = &import.grooves[0];
    assert_eq!(keys(groove), BTreeSet::from([KICK, SNARE, HAT]));
    assert_eq!(groove.bars[0].len(), 12);
    assert_eq!(groove.kit.as_deref(), Some("PopDrumKit"));
    assert_eq!(groove.similar_kit, Some(0));
}

/// A kit the table does not know keeps the keys 35 to 81 and leaves out the rest. So does a
/// channel with no kit chosen.
#[test]
fn unknown_kit_keeps_general_midi_keys() {
    for kits in [&[(RHYTHM, 0, 0)][..], &[]] {
        let mut notes = beat(1, RHYTHM, HAT);
        notes.push(note(2 * STEP, RHYTHM, 30, 70));
        notes.push(note(6 * STEP, RHYTHM, 82, 70));
        let bytes = style(setup(4, kits), vec![("Main A", 1, notes)]);
        let groove = &import("X.sty", &bytes).unwrap().grooves[0];
        assert_eq!(keys(groove), BTreeSet::from([KICK, SNARE, HAT]));
        assert_eq!(groove.kit, None);
        assert_eq!(groove.similar_kit, None);
    }
}

/// Only the kit of a channel that plays in the section is named.
#[test]
fn only_kits_that_play_are_named() {
    let bytes = style(
        setup(4, &[(RHYTHM, 127, 0), (SUB_RHYTHM, 126, 45)]),
        vec![
            ("Main A", 1, beat(1, RHYTHM, HAT)),
            ("Main B", 1, vec![note(0, SUB_RHYTHM, 24, 90)]),
        ],
    );
    let import = import("Pop.sty", &bytes).unwrap();
    assert_eq!(import.grooves[0].kit.as_deref(), Some("StandardKit1"));
    assert_eq!(import.grooves[1].kit.as_deref(), Some("PopPercussionKit"));
    assert_eq!(import.grooves[1].similar_kit, None);
}

/// A style in 3/4 has every section refused, each named after the style and the section.
#[test]
fn three_four_style_is_refused() {
    let bytes = style(
        setup(3, &[(RHYTHM, 127, 0)]),
        vec![
            ("Main A", 1, beat(1, RHYTHM, HAT)),
            ("Fill In AA", 1, beat(1, RHYTHM, HAT)),
        ],
    );
    let import = import("Waltz.prs", &bytes).unwrap();
    assert!(import.grooves.is_empty());
    assert_eq!(
        import.refused,
        vec![
            ("Waltz Main A".to_owned(), Refusal::NotFourFour),
            ("Waltz Fill In AA".to_owned(), Refusal::NotFourFour),
        ]
    );
}

/// Each section runs to the next one. Notes in the setup bar and on other channels are not part
/// of any groove, and other markers do not cut a section.
#[test]
fn sections_are_cut_at_their_markers() {
    let mut setup = setup(4, &[(RHYTHM, 127, 0)]);
    setup.push(note(0, RHYTHM, 49, 100));
    let mut main = beat(2, RHYTHM, HAT);
    main.push(note(0, 10, 60, 100)); // a bass note on channel 11
    main.push(marker(BAR, "SInt"));
    let bytes = style(
        setup,
        vec![
            ("Intro A", 1, vec![note(0, RHYTHM, 49, 100)]),
            ("Main A", 2, main),
            ("Ending A", 1, vec![note(0, RHYTHM, 57, 100)]),
        ],
    );
    let import = import("Rock.mid", &bytes).unwrap();
    let names: Vec<&str> = import.grooves.iter().map(|g| g.name.as_str()).collect();
    assert_eq!(names, ["Rock Intro A", "Rock Main A", "Rock Ending A"]);
    assert_eq!(keys(&import.grooves[0]), BTreeSet::from([49]));
    assert_eq!(import.grooves[1].bars.len(), 2);
    assert_eq!(keys(&import.grooves[1]), BTreeSet::from([KICK, SNARE, HAT]));
    assert_eq!(keys(&import.grooves[2]), BTreeSet::from([57]));
}

/// The blocks after the track (CASM, OTS, MDB) are not read, even when damaged.
#[test]
fn chunks_after_the_track_are_ignored() {
    let mut bytes = style(
        setup(4, &[(RHYTHM, 127, 0)]),
        vec![("Main A", 1, beat(1, RHYTHM, HAT))],
    );
    bytes.extend_from_slice(b"CASM");
    bytes.extend_from_slice(&1000u32.to_be_bytes());
    bytes.extend_from_slice(b"not what it claims");
    let import = import("Rock.sty", &bytes).unwrap();
    assert_eq!(import.refused, vec![]);
    assert_eq!(import.grooves.len(), 1);
}

/// A MIDI file without section markers is read as before, whatever its name.
#[test]
fn a_file_without_section_markers_is_not_a_style() {
    let bytes = smf(rock(1, HAT), BAR);
    assert!(!is_style(&bytes));
    let import = import("Beat.sty", &bytes).unwrap();
    assert_eq!(import.grooves[0].name, "Beat");
    assert_eq!(import.grooves[0].kit, None);
}

/// A zip may hold styles beside MIDI files. Each style's grooves go in a folder named after it.
#[test]
fn zip_with_a_style() {
    let bossa = style(
        setup(4, &[(RHYTHM, 127, 0)]),
        vec![
            ("Main A", 1, beat(1, RHYTHM, HAT)),
            ("Fill In AA", 1, vec![]),
        ],
    );
    let beat = smf(rock(1, HAT), BAR);
    let bytes = zip_of(&[
        ("Styles/Latin/Bossa.STY", &bossa),
        ("Styles/Beat.mid", &beat),
        ("Styles/readme.txt", b"hello"),
    ]);
    let import = import("Styles.zip", &bytes).unwrap();
    let grooves: Vec<(&str, &str)> = import
        .grooves
        .iter()
        .map(|g| (g.folder.as_str(), g.name.as_str()))
        .collect();
    assert_eq!(grooves, [("", "Beat"), ("Latin/Bossa", "Bossa Main A")]);
    assert_eq!(import.grooves[1].kit.as_deref(), Some("StandardKit1"));
    assert_eq!(
        import.refused,
        vec![("Styles/Latin/Bossa Fill In AA".to_owned(), Refusal::NoNotes)]
    );
}

/// The kit fields are kept in the pack file, and left out when empty.
#[test]
fn pack_json_keeps_the_kit() {
    let mut notes = beat(1, RHYTHM, HAT);
    notes.push(note(0, SUB_RHYTHM, 24, 90));
    let bytes = style(
        setup(4, &[(RHYTHM, 127, 0), (SUB_RHYTHM, 126, 45)]),
        vec![("Main A", 1, notes)],
    );
    let groove = import("Pop.sty", &bytes).unwrap().grooves.remove(0);
    let plain = one(&smf(rock(1, HAT), BAR)).unwrap();
    let pack = Pack {
        name: "Pop".into(),
        grooves: vec![groove, plain],
    };
    let text = pack_to_json(&pack);
    assert!(text.contains(r#""kit":"StandardKit1 + PopPercussionKit","similar_kit":0"#));
    assert_eq!(text.matches("kit").count(), 2, "{text}");
    assert_eq!(pack_from_json(&text), Ok(pack));
}

/// A pack written before the kit fields existed still reads, without a kit.
#[test]
fn pack_json_without_kit_fields_reads() {
    let text = r#"{"format":1,"name":"p","grooves":[{"name":"a","folder":"","bpm":100,"bars":[[[36,0,100]]]}]}"#;
    let pack = pack_from_json(text).unwrap();
    assert_eq!(pack.grooves[0].kit, None);
    assert_eq!(pack.grooves[0].similar_kit, None);
    let bad = text.replace(r#""bpm":100"#, r#""bpm":100,"similar_kit":128"#);
    assert!(pack_from_json(&bad).is_err());
}
