use super::*;
use crate::{DEFAULT_VELOCITY, STEPS_PER_BAR};

/// One decoded event: a meta event (type and data) or a channel message (all its bytes).
#[derive(Debug, Clone, PartialEq, Eq)]
enum Event {
    Meta(u8, Vec<u8>),
    Midi(Vec<u8>),
}

/// Reads a variable-length quantity at `*at`, moving past it.
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

/// Decodes a file written by `to_midi` into events with absolute ticks. Checks the header, the
/// track length and that the track ends with exactly one end-of-track event.
fn parse(bytes: &[u8]) -> Vec<(u32, Event)> {
    assert_eq!(&bytes[..14], b"MThd\0\0\0\x06\0\0\0\x01\0\x60", "header");
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
        } else {
            let len = match status & 0xF0 {
                0x80 | 0x90 | 0xB0 => 3,
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

/// The note-on events as (tick, key, velocity).
fn notes(bytes: &[u8]) -> Vec<(u32, u8, u8)> {
    parse(bytes)
        .into_iter()
        .filter_map(|(tick, e)| match e {
            Event::Midi(m) if m[0] == 0x99 => Some((tick, m[1], m[2])),
            _ => None,
        })
        .collect()
}

/// The tick of the end-of-track event.
fn end(bytes: &[u8]) -> u32 {
    parse(bytes).last().unwrap().0
}

fn on(p: &mut Project, index: usize, row: usize, step: usize) {
    p.set_step(index, row, step, DEFAULT_VELOCITY).unwrap();
}

const BAR: u32 = STEPS_PER_BAR as u32 * TICKS_PER_STEP;

#[test]
fn the_header_is_format_0_with_one_track_at_96_ticks() {
    let bytes = to_midi(&Project::new());
    assert_eq!(
        &bytes[..14],
        &[b'M', b'T', b'h', b'd', 0, 0, 0, 6, 0, 0, 0, 1, 0, 96]
    );
    assert_eq!(&bytes[14..18], b"MTrk");
    parse(&bytes);
}

#[test]
fn a_kick_and_a_snare_give_exactly_the_expected_events() {
    let mut p = Project::new();
    p.tempo = 120.0;
    on(&mut p, 0, 0, 0);
    on(&mut p, 0, 1, 4);
    let expected = vec![
        (0, Event::Meta(0x03, b"Drums".to_vec())),
        (0, Event::Meta(0x51, vec![0x07, 0xA1, 0x20])),
        (0, Event::Meta(0x58, vec![4, 2, 24, 8])),
        (0, Event::Midi(vec![0xC9, 0])),
        (0, Event::Midi(vec![0x99, 36, 100])),
        (12, Event::Midi(vec![0x89, 36, 64])),
        (96, Event::Midi(vec![0x99, 38, 100])),
        (108, Event::Midi(vec![0x89, 38, 64])),
        (384, Event::Meta(0x2F, vec![])),
    ];
    assert_eq!(parse(&to_midi(&p)), expected);
}

#[test]
fn the_tempo_is_written_in_microseconds_per_beat() {
    let tempo = |bpm: f32| {
        let mut p = Project::new();
        p.tempo = bpm;
        parse(&to_midi(&p))
            .into_iter()
            .find_map(|(_, e)| match e {
                Event::Meta(0x51, data) => Some(u32::from_be_bytes([0, data[0], data[1], data[2]])),
                _ => None,
            })
            .unwrap()
    };
    assert_eq!(tempo(120.0), 500_000);
    assert_eq!(tempo(100.0), 600_000);
    assert_eq!(tempo(90.0), 666_667);
    assert_eq!(tempo(40.0), 1_500_000);
    assert_eq!(
        tempo(0.0),
        600_000,
        "an impossible tempo counts as the default"
    );
}

#[test]
fn the_kit_is_a_program_change_on_channel_10() {
    let mut p = Project::new();
    p.set_kit(25);
    let events = parse(&to_midi(&p));
    assert!(events.contains(&(0, Event::Midi(vec![0xC9, 25]))));
    assert!(
        !events
            .iter()
            .any(|(_, e)| matches!(e, Event::Midi(m) if m[0] & 0xF0 == 0xB0)),
        "no Bank Select"
    );
}

#[test]
fn muted_rows_are_left_out_and_volume_scales_velocity() {
    let mut p = Project::new();
    on(&mut p, 0, 0, 0);
    on(&mut p, 0, 1, 4);
    p.set_step(0, 2, 8, 1).unwrap();
    p.toggle_mute(1).unwrap();
    p.set_volume(0, 50).unwrap();
    p.set_volume(2, 10).unwrap();
    // 100 at 50% is 50; 1 at 10% rounds to 0 and is raised to the lowest audible velocity, 1.
    assert_eq!(notes(&to_midi(&p)), vec![(0, 36, 50), (192, 42, 1)]);
}

#[test]
fn only_soloed_rows_play_while_any_row_is_soloed() {
    let mut p = Project::new();
    on(&mut p, 0, 0, 0);
    on(&mut p, 0, 1, 4);
    on(&mut p, 0, 2, 8);
    p.toggle_solo(1).unwrap();
    assert_eq!(notes(&to_midi(&p)), vec![(96, 38, 100)]);
}

#[test]
fn swing_delays_odd_steps() {
    let mut p = Project::new();
    for step in 0..4 {
        on(&mut p, 0, 2, step);
    }
    p.set_swing(0.25);
    let ticks: Vec<u32> = notes(&to_midi(&p)).iter().map(|n| n.0).collect();
    assert_eq!(ticks, vec![0, 30, 48, 78]);
    p.set_swing(0.33);
    let ticks: Vec<u32> = notes(&to_midi(&p)).iter().map(|n| n.0).collect();
    assert_eq!(ticks, vec![0, 32, 48, 80], "0.33 * 24 = 7.92 rounds to 8");
}

#[test]
fn a_note_ending_where_the_next_starts_is_turned_off_first() {
    let mut p = Project::new();
    on(&mut p, 0, 2, 1);
    on(&mut p, 0, 2, 2);
    p.set_swing(MAX_SWING);
    // Step 1 starts at 36 and ends at 48, where step 2 starts.
    let at_48: Vec<Event> = parse(&to_midi(&p))
        .into_iter()
        .filter(|(tick, _)| *tick == 48)
        .map(|(_, e)| e)
        .collect();
    assert_eq!(
        at_48,
        vec![
            Event::Midi(vec![0x89, 42, 64]),
            Event::Midi(vec![0x99, 42, 100])
        ]
    );
}

#[test]
fn the_song_plays_once_in_order_without_hidden_bars() {
    let mut p = Project::new();
    on(&mut p, 0, 0, 0);
    // A hidden bar in A, which must not play.
    p.add_bar(0).unwrap();
    on(&mut p, 0, 3, STEPS_PER_BAR);
    p.remove_bar(0).unwrap();
    let (b, _) = p.add_pattern().unwrap();
    p.add_bar(b).unwrap();
    on(&mut p, b, 1, STEPS_PER_BAR);
    p.song_append(b).unwrap();
    p.song_append(0).unwrap();
    // A (1 bar), B (2 bars), A (1 bar).
    let bytes = to_midi(&p);
    assert_eq!(
        notes(&bytes),
        vec![(0, 36, 100), (2 * BAR, 38, 100), (3 * BAR, 36, 100)]
    );
    assert_eq!(end(&bytes), 4 * BAR);
}

#[test]
fn an_empty_song_plays_the_first_pattern_once() {
    let mut p = Project::new();
    on(&mut p, 0, 0, 4);
    let (b, _) = p.add_pattern().unwrap();
    on(&mut p, b, 1, 0);
    p.song_remove(0).unwrap();
    assert!(p.song().is_empty());
    let bytes = to_midi(&p);
    assert_eq!(notes(&bytes), vec![(96, 36, 100)]);
    assert_eq!(end(&bytes), BAR);
}

#[test]
fn the_track_ends_at_the_end_of_the_song_even_with_full_swing() {
    let mut p = Project::new();
    on(&mut p, 0, 0, STEPS_PER_BAR - 1);
    p.set_swing(MAX_SWING);
    // Step 15 starts at 15 * 24 + 12 = 372 and ends at 384, exactly the end of the bar.
    assert_eq!(end(&to_midi(&p)), BAR);
    p.set_swing(0.0);
    assert_eq!(end(&to_midi(&p)), BAR);
}

#[test]
fn variable_length_quantities_are_encoded() {
    let vlq = |value| {
        let mut out = Vec::new();
        write_vlq(&mut out, value);
        out
    };
    assert_eq!(vlq(0), vec![0x00]);
    assert_eq!(vlq(127), vec![0x7F]);
    assert_eq!(vlq(128), vec![0x81, 0x00]);
    assert_eq!(vlq(16383), vec![0xFF, 0x7F]);
    assert_eq!(vlq(16384), vec![0x81, 0x80, 0x00]);
    assert_eq!(vlq(0x0FFF_FFFF), vec![0xFF, 0xFF, 0xFF, 0x7F]);
}
