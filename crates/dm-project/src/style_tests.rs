use super::yamaha::{TICKS_PER_BAR, TICKS_PER_STEP};
use super::*;
use crate::{DEFAULT_VELOCITY, MAX_SWING, preset_project};

/// One decoded event: a meta event (type and data), a system exclusive message (its data after
/// the length) or a channel message (all its bytes).
#[derive(Debug, Clone, PartialEq, Eq)]
enum Event {
    Meta(u8, Vec<u8>),
    Sysex(Vec<u8>),
    Midi(Vec<u8>),
}

fn read_vlq(bytes: &[u8], at: &mut usize) -> u32 {
    let mut value = 0;
    loop {
        let byte = bytes[*at];
        *at += 1;
        value = (value << 7) | u32::from(byte & 0x7F);
        if byte & 0x80 == 0 {
            return value;
        }
    }
}

/// Decodes a style into events with absolute ticks. Checks that the header is format 0 with one
/// track at 1920 ticks per beat, the track length, and that the track ends with one end of track.
fn parse(bytes: &[u8]) -> Vec<(u32, Event)> {
    assert_eq!(&bytes[..4], b"MThd");
    assert_eq!(
        &bytes[4..14],
        &[0, 0, 0, 6, 0, 0, 0, 1, 0x07, 0x80],
        "header"
    );
    assert_eq!(&bytes[14..18], b"MTrk");
    let length = u32::from_be_bytes(bytes[18..22].try_into().unwrap()) as usize;
    assert_eq!(length, bytes.len() - 22, "track chunk length");
    let mut at = 22;
    let mut tick = 0;
    let mut events = Vec::new();
    while at < bytes.len() {
        tick += read_vlq(bytes, &mut at);
        let status = bytes[at];
        let event = if status == 0xFF {
            let kind = bytes[at + 1];
            at += 2;
            let len = read_vlq(bytes, &mut at) as usize;
            let data = bytes[at..at + len].to_vec();
            at += len;
            Event::Meta(kind, data)
        } else if status == 0xF0 {
            at += 1;
            let len = read_vlq(bytes, &mut at) as usize;
            let data = bytes[at..at + len].to_vec();
            at += len;
            Event::Sysex(data)
        } else {
            let len = match status & 0xF0 {
                0x90 | 0xB0 => 3,
                0xC0 => 2,
                other => panic!("unexpected status {other:#x}"),
            };
            let data = bytes[at..at + len].to_vec();
            at += len;
            Event::Midi(data)
        };
        events.push((tick, event));
    }
    let ends = events
        .iter()
        .filter(|(_, e)| *e == Event::Meta(0x2F, vec![]))
        .count();
    assert_eq!(ends, 1);
    assert_eq!(events.last().unwrap().1, Event::Meta(0x2F, vec![]));
    events
}

/// The section markers as (tick, name), leaving out the setup markers.
fn markers(bytes: &[u8]) -> Vec<(u32, String)> {
    parse(bytes)
        .into_iter()
        .filter_map(|(tick, e)| match e {
            Event::Meta(0x06, data) if tick > 0 => Some((tick, String::from_utf8(data).unwrap())),
            _ => None,
        })
        .collect()
}

/// The note ons as (tick, key, velocity).
fn notes(bytes: &[u8]) -> Vec<(u32, u8, u8)> {
    parse(bytes)
        .into_iter()
        .filter_map(|(tick, e)| match e {
            Event::Midi(m) if m[0] == 0x99 && m[2] > 0 => Some((tick, m[1], m[2])),
            _ => None,
        })
        .collect()
}

/// The style's name, meta event 3.
fn name_in(bytes: &[u8]) -> String {
    parse(bytes)
        .into_iter()
        .find_map(|(_, e)| match e {
            Event::Meta(0x03, data) => Some(String::from_utf8(data).unwrap()),
            _ => None,
        })
        .unwrap()
}

fn on(p: &mut Project, index: usize, row: usize, step: usize) {
    p.set_step(index, row, step, DEFAULT_VELOCITY).unwrap();
}

/// Only Main A, from the first pattern.
fn main_a_only() -> StyleSections {
    StyleSections {
        mains: [Some(0), None, None, None],
        ..StyleSections::default()
    }
}

const BAR: u32 = TICKS_PER_BAR;
const STEP: u32 = TICKS_PER_STEP;

#[test]
fn the_rock_preset_suggests_a_main_and_its_fill() {
    let p = preset_project("Rock").unwrap();
    let expected = StyleSections {
        mains: [Some(0), None, None, None],
        fills: [Some(1), None, None, None],
        ..StyleSections::default()
    };
    assert_eq!(StyleSections::suggested(&p), expected);
}

#[test]
fn without_a_song_the_patterns_own_order_is_used() {
    let mut p = Project::new();
    p.add_bar(0).unwrap(); // A: two bars
    p.add_pattern().unwrap(); // B: one bar
    p.add_pattern().unwrap(); // C: one bar
    p.add_pattern().unwrap(); // D: one bar
    p.song.clear();
    // A is a main and B its fill. C follows a fill, so it is Main B, and D is its fill.
    let expected = StyleSections {
        mains: [Some(0), Some(2), None, None],
        fills: [Some(1), Some(3), None, None],
        ..StyleSections::default()
    };
    assert_eq!(StyleSections::suggested(&p), expected);
}

#[test]
fn the_suggestion_follows_the_song_and_takes_each_pattern_once() {
    let mut p = Project::new();
    for _ in 0..6 {
        p.add_pattern().unwrap();
    }
    for index in [1, 2, 3] {
        p.add_bar(index).unwrap();
    }
    // Slots 0, 4, 5 and 6 are one bar; 1, 2 and 3 are two bars.
    p.song = vec![1, 1, 0, 2, 2, 1, 4, 3, 5, 6];
    // B is Main A and A, right after it, its fill. C is Main B. B again comes before E, but B
    // already has a fill, so E is Main C. D is Main D and F its fill. G is one too many.
    let expected = StyleSections {
        mains: [Some(1), Some(2), Some(4), Some(3)],
        fills: [Some(0), None, None, Some(5)],
        ..StyleSections::default()
    };
    assert_eq!(StyleSections::suggested(&p), expected);
}

#[test]
fn main_a_is_required() {
    let p = Project::new();
    let sections = StyleSections {
        mains: [None, Some(0), None, None],
        ..StyleSections::default()
    };
    assert_eq!(
        KeyboardKind::Yamaha.problem(&p, &sections),
        Some(StyleProblem::NoMainA)
    );
    assert_eq!(
        KeyboardKind::Yamaha.write(&p, &sections, "x"),
        Err(StyleProblem::NoMainA)
    );
    assert_eq!(KeyboardKind::Yamaha.problem(&p, &main_a_only()), None);
}

#[test]
fn fills_and_the_break_must_be_one_bar() {
    let mut p = Project::new();
    p.add_pattern().unwrap();
    p.add_bar(1).unwrap(); // B: two bars
    let fill = StyleSections {
        fills: [Some(1), None, None, None],
        ..main_a_only()
    };
    assert_eq!(
        KeyboardKind::Yamaha.problem(&p, &fill),
        Some(StyleProblem::NotOneBar)
    );
    let fill_break = StyleSections {
        fill_break: Some(1),
        ..main_a_only()
    };
    assert_eq!(
        KeyboardKind::Yamaha.problem(&p, &fill_break),
        Some(StyleProblem::NotOneBar)
    );
    // A two-bar main, intro or ending is fine.
    let long = StyleSections {
        intro: Some(1),
        mains: [Some(1), Some(1), None, None],
        ending: Some(1),
        ..StyleSections::default()
    };
    assert_eq!(KeyboardKind::Yamaha.problem(&p, &long), None);
}

#[test]
fn a_fill_whose_main_is_left_out_is_not_checked() {
    let mut p = Project::new();
    p.add_pattern().unwrap();
    p.add_bar(1).unwrap();
    let sections = StyleSections {
        fills: [None, Some(1), Some(9), None],
        ..main_a_only()
    };
    assert_eq!(KeyboardKind::Yamaha.problem(&p, &sections), None);
    let bytes = KeyboardKind::Yamaha.write(&p, &sections, "x").unwrap();
    let names: Vec<String> = markers(&bytes).into_iter().map(|(_, m)| m).collect();
    assert_eq!(names, ["Main A", "Fill In AA"]);
}

#[test]
fn a_missing_pattern_is_a_problem() {
    let p = Project::new();
    let sections = StyleSections {
        ending: Some(7),
        ..main_a_only()
    };
    assert_eq!(
        KeyboardKind::Yamaha.problem(&p, &sections),
        Some(StyleProblem::NoSuchPattern)
    );
}

#[test]
fn the_sections_come_in_order_each_on_a_bar_line() {
    let mut p = Project::new();
    p.add_bar(0).unwrap(); // A: two bars
    p.add_pattern().unwrap(); // B: one bar
    p.add_pattern().unwrap(); // C: three bars
    p.add_bar(2).unwrap();
    p.add_bar(2).unwrap();
    let sections = StyleSections {
        intro: Some(2),
        mains: [Some(0), Some(2), None, Some(1)],
        fills: [Some(1), None, Some(1), None],
        fill_break: Some(1),
        ending: Some(0),
    };
    let bytes = KeyboardKind::Yamaha.write(&p, &sections, "x").unwrap();
    let expected: Vec<(u32, String)> = [
        (1, "Intro A"),
        (4, "Main A"),
        (6, "Fill In AA"),
        (7, "Main B"),
        (10, "Fill In BB"),
        (11, "Main D"),
        (12, "Fill In DD"),
        (13, "Fill In BA"),
        (14, "Ending A"),
    ]
    .into_iter()
    .map(|(bar, name)| (bar * BAR, name.to_owned()))
    .collect();
    assert_eq!(markers(&bytes), expected);
    assert_eq!(parse(&bytes).last().unwrap().0, 16 * BAR, "end of track");
}

#[test]
fn the_setup_bar_selects_the_kit_on_the_drum_channels() {
    let mut p = Project::new();
    p.set_kit(25);
    p.tempo = 120.0;
    let bytes = KeyboardKind::Yamaha
        .write(&p, &main_a_only(), "Beat")
        .unwrap();
    let setup: Vec<Event> = parse(&bytes)
        .into_iter()
        .take_while(|(tick, _)| *tick == 0)
        .map(|(_, e)| e)
        .collect();
    let mut expected = vec![
        Event::Meta(0x58, vec![4, 2, 24, 8]),
        Event::Meta(0x51, vec![0x07, 0xA1, 0x20]),
        Event::Meta(0x06, b"SFF1".to_vec()),
        Event::Meta(0x03, b"Beat".to_vec()),
        Event::Meta(0x06, b"SInt".to_vec()),
        Event::Sysex(vec![0x7E, 0x7F, 0x09, 0x01, 0xF7]),
    ];
    for channel in [8, 9] {
        expected.extend([
            Event::Midi(vec![0xB0 | channel, 0, 127]),
            Event::Midi(vec![0xB0 | channel, 32, 0]),
            Event::Midi(vec![0xC0 | channel, 25]),
            Event::Midi(vec![0xB0 | channel, 7, 100]),
            Event::Midi(vec![0xB0 | channel, 10, 64]),
            Event::Midi(vec![0xB0 | channel, 91, 40]),
            Event::Midi(vec![0xB0 | channel, 93, 0]),
        ]);
    }
    assert_eq!(setup, expected);
}

#[test]
fn mute_solo_and_volume_apply_to_the_notes() {
    let mut p = Project::new();
    on(&mut p, 0, 0, 0); // kick
    on(&mut p, 0, 1, 4); // snare
    on(&mut p, 0, 2, 8); // closed hat
    p.set_volume(1, 50).unwrap();
    p.toggle_mute(2).unwrap();
    let bytes = KeyboardKind::Yamaha.write(&p, &main_a_only(), "x").unwrap();
    // Main A, then its last bar again as the fill.
    let expected = vec![
        (BAR, 36, 100),
        (BAR + 4 * STEP, 38, 50),
        (2 * BAR, 36, 100),
        (2 * BAR + 4 * STEP, 38, 50),
    ];
    assert_eq!(notes(&bytes), expected);

    p.toggle_solo(1).unwrap();
    let bytes = KeyboardKind::Yamaha.write(&p, &main_a_only(), "x").unwrap();
    assert_eq!(
        notes(&bytes),
        [(BAR + 4 * STEP, 38, 50), (2 * BAR + 4 * STEP, 38, 50)]
    );
}

#[test]
fn a_main_without_a_fill_uses_its_last_bar_and_hidden_bars_are_left_out() {
    let mut p = Project::new();
    p.add_bar(0).unwrap();
    p.add_bar(0).unwrap();
    on(&mut p, 0, 0, 0); // bar 1
    on(&mut p, 0, 1, STEPS_PER_BAR + 2); // bar 2
    on(&mut p, 0, 2, 2 * STEPS_PER_BAR + 4); // bar 3, hidden below
    p.remove_bar(0).unwrap();
    let bytes = KeyboardKind::Yamaha.write(&p, &main_a_only(), "x").unwrap();
    assert_eq!(
        markers(&bytes),
        [
            (BAR, "Main A".to_owned()),
            (3 * BAR, "Fill In AA".to_owned())
        ]
    );
    let expected = vec![
        (BAR, 36, 100),
        (2 * BAR + 2 * STEP, 38, 100),
        (3 * BAR + 2 * STEP, 38, 100),
    ];
    assert_eq!(notes(&bytes), expected);
    assert_eq!(parse(&bytes).last().unwrap().0, 4 * BAR);
}

#[test]
fn with_most_swing_the_last_note_ends_on_the_section_end_before_the_next_marker() {
    let mut p = Project::new();
    p.set_swing(MAX_SWING);
    on(&mut p, 0, 0, STEPS_PER_BAR - 1);
    let bytes = KeyboardKind::Yamaha.write(&p, &main_a_only(), "x").unwrap();
    let events = parse(&bytes);
    let late = BAR + 15 * STEP + STEP / 2;
    assert!(events.contains(&(late, Event::Midi(vec![0x99, 36, 100]))));
    let boundary: Vec<&Event> = events
        .iter()
        .filter(|(tick, _)| *tick == 2 * BAR)
        .map(|(_, e)| e)
        .collect();
    assert_eq!(
        boundary,
        [
            &Event::Midi(vec![0x99, 36, 0]),
            &Event::Meta(0x06, b"Fill In AA".to_vec()),
        ]
    );
}

#[test]
fn a_note_off_never_passes_its_section_end() {
    // Swing is at most half a step, so the note off lands at most on the end; check that no
    // note off of any section is later than the next marker.
    let mut p = preset_project("Reggae").unwrap();
    p.set_swing(MAX_SWING);
    for step in 0..STEPS_PER_BAR {
        on(&mut p, 0, 3, step);
    }
    let bytes = KeyboardKind::Yamaha.write(&p, &main_a_only(), "x").unwrap();
    let ends: Vec<u32> = markers(&bytes)
        .into_iter()
        .skip(1)
        .map(|(tick, _)| tick)
        .chain([parse(&bytes).last().unwrap().0])
        .collect();
    let mut open = 0;
    let mut section = 0;
    for (tick, event) in parse(&bytes) {
        if let Event::Meta(0x06, _) = event
            && tick > BAR
        {
            assert_eq!(open, 0, "every note is off before the marker at {tick}");
            section += 1;
        }
        if let Event::Midi(m) = event
            && m[0] == 0x99
        {
            assert!(tick <= ends[section]);
            if m[2] > 0 {
                open += 1;
            } else {
                open -= 1;
            }
        }
    }
    assert_eq!(open, 0);
}

#[test]
fn the_name_is_plain_ascii() {
    let p = Project::new();
    let name = |n: &str| name_in(&KeyboardKind::Yamaha.write(&p, &main_a_only(), n).unwrap());
    assert_eq!(name("Ação Éxito ñ"), "Acao Exito n");
    assert_eq!(name("Groove 2"), "Groove 2");
    assert_eq!(name("日本 ß\t!"), "__ __!");
    assert_eq!(name(""), "TickTapDrum");
    assert_eq!(name("   "), "TickTapDrum");
}

/// What `cargo run -p dm-project --example yamaha_style -- out.sty` writes: the SFF1 style the
/// PSR-SX720 loaded and played, named "TTD Test". Regenerate it with that command if the example
/// changes.
const EXAMPLE: &str = "\
4d546864000000060000000107804d54726b0000020600ff58040402180800ff51030927c000ff0604534646310\
0ff0308545444205465737400ff060453496e7400f0057e7f0901f700b8007f00b8200000c80000b8076400b80a\
4000b85b2800b85d0000b9007f00b9200000c90000b9076400b90a4000b95b2800b95d00bc00ff06064d61696e2\
04100992a640099246e8170992a00009924008550992a468170992a008550992a640099266e8170992a00009926\
008550992a468170992a008550992a640099246e8170992a00009924008550992a460099246e8170992a0000992\
4008550992a640099266e8170992a00009926008550992a468170992a008550992a640099246e8170992a000099\
24008550992a468170992a008550992a640099266e8170992a00009926008550992a468170992a008550992a640\
099246e8170992a00009924008550992a460099246e8170992a00009924008550992a640099266e8170992a0000\
9926008550992e468170992e008550ff060a46696c6c20496e20414100992a640099246e8170992a00009924008\
550992a468170992a008550992a640099266e8170992a00009926008550992a468170992a008550992650817099\
2600817099265a817099260081709926648170992600817099266e8170992600817099326e0099246e817099320\
0009924008170992f6e8170992f008170992d6e8170992d008170992b6e8170992b008170ff2f00";

#[test]
fn the_example_project_gives_the_same_bytes_as_the_tried_example() {
    const CLOSED_HAT: usize = 0;
    const OPEN_HAT: usize = 1;
    const HIGH_TOM: usize = 2;
    const MID_TOM: usize = 3;
    const LOW_TOM: usize = 4;
    const FLOOR_TOM: usize = 5;
    const KICK: usize = 6;
    const SNARE: usize = 7;
    let mut p = Project::new();
    // The example writes hits at one tick in this order, so the rows follow it.
    for (row, key) in [42, 46, 50, 47, 45, 43, 36, 38].into_iter().enumerate() {
        p.rows[row].key = key;
    }
    let hit = |p: &mut Project, index: usize, row: usize, step: usize, velocity: u8| {
        p.set_step(index, row, step, velocity).unwrap();
    };
    // Main A: two bars of a rock beat.
    p.add_bar(0).unwrap();
    for bar in 0..2 {
        let at = bar * STEPS_PER_BAR;
        for step in (0..STEPS_PER_BAR).step_by(2) {
            let accent = if step % 4 == 0 { 100 } else { 70 };
            let open = bar == 1 && step == 14;
            hit(
                &mut p,
                0,
                if open { OPEN_HAT } else { CLOSED_HAT },
                at + step,
                accent,
            );
        }
        for step in [0, 8, 10] {
            hit(&mut p, 0, KICK, at + step, 110);
        }
        for step in [4, 12] {
            hit(&mut p, 0, SNARE, at + step, 110);
        }
    }
    // Fill In AA: half a beat of hats, then snare and toms down the kit.
    p.add_pattern().unwrap();
    for (step, velocity) in [(0, 100), (2, 70), (4, 100), (6, 70)] {
        hit(&mut p, 1, CLOSED_HAT, step, velocity);
    }
    hit(&mut p, 1, KICK, 0, 110);
    hit(&mut p, 1, KICK, 12, 110);
    hit(&mut p, 1, SNARE, 4, 110);
    for step in 8..12 {
        hit(&mut p, 1, SNARE, step, 80 + (step as u8 - 8) * 10);
    }
    for (step, row) in [
        (12, HIGH_TOM),
        (13, MID_TOM),
        (14, LOW_TOM),
        (15, FLOOR_TOM),
    ] {
        hit(&mut p, 1, row, step, 110);
    }

    p.song = vec![0, 1];
    let sections = StyleSections::suggested(&p);
    assert_eq!(sections.mains[0], Some(0));
    assert_eq!(sections.fills[0], Some(1));
    let bytes = KeyboardKind::Yamaha
        .write(&p, &sections, "TTD Test")
        .unwrap();
    let expected: Vec<u8> = (0..EXAMPLE.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&EXAMPLE[i..i + 2], 16).unwrap())
        .collect();
    assert_eq!(bytes, expected);
}

#[test]
fn get_and_set_reach_every_section() {
    let mut sections = StyleSections::default();
    for (i, section) in Section::ALL.into_iter().enumerate() {
        sections.set(section, Some(i));
    }
    for (i, section) in Section::ALL.into_iter().enumerate() {
        assert_eq!(sections.get(section), Some(i));
    }
    // An index past Main D is ignored rather than a panic.
    sections.set(Section::Main(4), Some(99));
    assert_eq!(sections.get(Section::Main(4)), None);
}

#[test]
fn the_file_name_is_safe_for_the_keyboard() {
    let yamaha = KeyboardKind::Yamaha;
    assert_eq!(yamaha.file_name("Minha batida"), "Minha batida.sty");
    assert_eq!(yamaha.file_name("Canção: nº 1?"), "Cancao_ n_ 1_.sty");
    assert_eq!(yamaha.file_name("a/b\\c"), "a_b_c.sty");
    assert_eq!(yamaha.file_name("  "), "TickTapDrum.sty");
}

#[test]
fn yamaha_has_every_section_and_one_bar_fills() {
    let yamaha = KeyboardKind::Yamaha;
    assert_eq!(yamaha.extension(), "sty");
    assert!(Section::ALL.into_iter().all(|s| yamaha.has(s)));
    assert!(yamaha.one_bar(Section::Fill(2)));
    assert!(yamaha.one_bar(Section::Break));
    assert!(!yamaha.one_bar(Section::Main(0)));
    assert!(!yamaha.one_bar(Section::Intro));
}
