use std::collections::BTreeSet;
use std::io::{Cursor, Write};

use zip::write::SimpleFileOptions;

use super::*;

const PPQ: u16 = 96;
/// Ticks in one step at [`PPQ`].
const STEP: u32 = PPQ as u32 / 4;
const BAR: u32 = STEP * 16;
const KICK: u8 = 36;
const SNARE: u8 = 38;
const HAT: u8 = 42;

/// A MIDI event at a tick, without its delta time.
struct Event(u32, Vec<u8>);

fn note(tick: u32, channel: u8, key: u8, velocity: u8) -> Event {
    Event(tick, vec![0x90 | channel, key, velocity])
}

fn tempo(tick: u32, bpm: f64) -> Event {
    let micros = (60_000_000.0 / bpm).round() as u32;
    let [_, a, b, c] = micros.to_be_bytes();
    Event(tick, vec![0xFF, 0x51, 3, a, b, c])
}

fn time_signature(tick: u32, numerator: u8, denominator_power: u8) -> Event {
    Event(
        tick,
        vec![0xFF, 0x58, 4, numerator, denominator_power, 24, 8],
    )
}

/// A format 0 file holding the events, ending at `end` or the last event if later.
fn smf(events: Vec<Event>, end: u32) -> Vec<u8> {
    let mut events = events;
    events.sort_by_key(|e| e.0);
    let mut track = Vec::new();
    let mut previous = 0;
    for Event(tick, bytes) in &events {
        vlq(&mut track, tick - previous);
        track.extend_from_slice(bytes);
        previous = *tick;
    }
    vlq(&mut track, end.saturating_sub(previous));
    track.extend_from_slice(&[0xFF, 0x2F, 0]);
    let mut file = b"MThd".to_vec();
    file.extend_from_slice(&6u32.to_be_bytes());
    file.extend_from_slice(&0u16.to_be_bytes());
    file.extend_from_slice(&1u16.to_be_bytes());
    file.extend_from_slice(&PPQ.to_be_bytes());
    file.extend_from_slice(b"MTrk");
    file.extend_from_slice(&(track.len() as u32).to_be_bytes());
    file.extend_from_slice(&track);
    file
}

fn vlq(out: &mut Vec<u8>, mut value: u32) {
    let mut bytes = vec![(value & 0x7F) as u8];
    value >>= 7;
    while value > 0 {
        bytes.push((value & 0x7F) as u8 | 0x80);
        value >>= 7;
    }
    out.extend(bytes.iter().rev());
}

/// A plain rock beat on channel 10: hats on every 8th, kick on 1 and 3, snare on 2 and 4.
fn rock(bars: u32, hat: u8) -> Vec<Event> {
    let mut events = vec![tempo(0, 100.0), time_signature(0, 4, 2)];
    for bar in 0..bars {
        let at = bar * BAR;
        for step in (0..16).step_by(2) {
            events.push(note(at + step * STEP, 9, hat, 80));
        }
        for step in [0, 8] {
            events.push(note(at + step * STEP, 9, KICK, 110));
        }
        for step in [4, 12] {
            events.push(note(at + step * STEP, 9, SNARE, 100));
        }
    }
    events
}

fn one(bytes: &[u8]) -> Result<Groove, Refusal> {
    let import = import("Beat.mid", bytes).unwrap();
    assert_eq!(import.grooves.len() + import.refused.len(), 1);
    match import.grooves.into_iter().next() {
        Some(groove) => Ok(groove),
        None => Err(import.refused[0].1),
    }
}

fn hits(bar: &[BarHit]) -> BTreeSet<(u8, usize, u8)> {
    bar.iter().map(|h| (h.key, h.step, h.velocity)).collect()
}

fn zip_of(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in files {
        writer
            .start_file(*name, SimpleFileOptions::default())
            .unwrap();
        writer.write_all(bytes).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

/// Every built-in preset, exported with the app's MIDI writer, imports with the same hits.
#[test]
fn presets_round_trip() {
    for preset in dm_project::presets() {
        let project = dm_project::preset_project(preset.name).unwrap();
        let mut expected = BTreeSet::new();
        let mut at = 0;
        for &slot in project.song() {
            let pattern = &project.patterns()[project.index_of_slot(slot).unwrap()];
            for (row, info) in project.rows().iter().enumerate() {
                for step in 0..pattern.length() {
                    let v = pattern.velocity(row, step);
                    if v > 0 {
                        expected.insert((at + step / 16, info.key, step % 16, v));
                    }
                }
            }
            at += pattern.bars();
        }
        let groove = one(&dm_project::to_midi(&project))
            .unwrap_or_else(|r| panic!("{} refused: {r:?}", preset.name));
        let got: BTreeSet<_> = groove
            .bars
            .iter()
            .enumerate()
            .flat_map(|(bar, hits)| hits.iter().map(move |h| (bar, h.key, h.step, h.velocity)))
            .collect();
        assert_eq!(got, expected, "{}", preset.name);
        assert_eq!(groove.bars.len(), at, "{}", preset.name);
        assert_eq!(groove.bpm, project.tempo.round(), "{}", preset.name);
    }
}

#[test]
fn plain_groove_is_accepted() {
    let groove = one(&smf(rock(2, HAT), 2 * BAR)).unwrap();
    assert_eq!(groove.name, "Beat");
    assert_eq!(groove.folder, "");
    assert_eq!(groove.bpm, 100.0);
    assert_eq!(groove.bars.len(), 2);
    assert_eq!(groove.bars[0].len(), 12);
    assert_eq!(hits(&groove.bars[0]), hits(&groove.bars[1]));
    assert!(groove.bars[0].contains(&BarHit {
        key: SNARE,
        step: 4,
        velocity: 100
    }));
}

#[test]
fn no_tempo_and_no_time_signature_mean_120_and_four_four() {
    let events = vec![note(0, 9, KICK, 100), note(8 * STEP, 9, SNARE, 100)];
    let groove = one(&smf(events, BAR)).unwrap();
    assert_eq!(groove.bpm, 120.0);
    assert_eq!(groove.bars.len(), 1);
}

#[test]
fn tempo_is_rounded_and_clamped() {
    let mut events = rock(1, HAT);
    events[0] = tempo(0, 109.99991);
    assert_eq!(one(&smf(events, BAR)).unwrap().bpm, 110.0);
    let mut events = rock(1, HAT);
    events[0] = tempo(0, 300.0);
    assert_eq!(one(&smf(events, BAR)).unwrap().bpm, 240.0);
}

#[test]
fn one_channel_other_than_ten_is_fine() {
    let events = vec![note(0, 0, KICK, 100), note(8 * STEP, 0, SNARE, 100)];
    assert!(one(&smf(events, BAR)).is_ok());
}

#[test]
fn three_four_is_refused() {
    let mut events = rock(1, HAT);
    events[1] = time_signature(0, 3, 2);
    assert_eq!(one(&smf(events, BAR)), Err(Refusal::NotFourFour));
}

#[test]
fn change_of_time_signature_is_refused() {
    let mut events = rock(2, HAT);
    events.push(time_signature(BAR, 6, 3));
    assert_eq!(one(&smf(events, 2 * BAR)), Err(Refusal::NotFourFour));
}

#[test]
fn two_channels_are_refused() {
    let mut events = rock(1, HAT);
    events.push(note(0, 0, 60, 100));
    assert_eq!(one(&smf(events, BAR)), Err(Refusal::SeveralChannels));
}

#[test]
fn tempo_change_is_refused() {
    let mut events = rock(2, HAT);
    events.push(tempo(BAR, 140.0));
    assert_eq!(one(&smf(events, 2 * BAR)), Err(Refusal::TempoChanges));
}

#[test]
fn repeated_tempo_is_not_a_change() {
    let mut events = rock(2, HAT);
    events.push(tempo(BAR, 100.001));
    assert!(one(&smf(events, 2 * BAR)).is_ok());
}

#[test]
fn twenty_bars_are_refused() {
    assert_eq!(one(&smf(rock(20, HAT), 20 * BAR)), Err(Refusal::TooLong));
    assert!(one(&smf(rock(16, HAT), 16 * BAR)).is_ok());
}

#[test]
fn no_notes_are_refused() {
    assert_eq!(one(&smf(vec![tempo(0, 100.0)], BAR)), Err(Refusal::NoNotes));
}

#[test]
fn hi_hat_on_a_conga_key_is_another_drum_program() {
    // Perfect Drums puts its hi-hat on key 63, the open high conga in General MIDI.
    assert_eq!(one(&smf(rock(1, 63), BAR)), Err(Refusal::OtherDrumProgram));
}

#[test]
fn hi_hat_on_the_tambourine_key_is_another_drum_program() {
    // GetGood Drums puts a hi-hat on key 54, the tambourine, inside the kit's key range.
    assert_eq!(one(&smf(rock(1, 54), BAR)), Err(Refusal::OtherDrumProgram));
}

#[test]
fn key_outside_general_midi_is_another_drum_program() {
    let mut events = rock(4, HAT);
    events.push(note(0, 9, 10, 100));
    assert_eq!(one(&smf(events, 4 * BAR)), Err(Refusal::OtherDrumProgram));
}

#[test]
fn td11_keys_are_mapped() {
    // The Groove MIDI Dataset's hi-hat edge is key 22.
    let groove = one(&smf(rock(1, 22), BAR)).unwrap();
    let keys: BTreeSet<u8> = groove.bars[0].iter().map(|h| h.key).collect();
    assert_eq!(keys, BTreeSet::from([KICK, SNARE, HAT]));
}

#[test]
fn humanised_notes_snap_to_the_grid() {
    let plain = one(&smf(rock(2, HAT), 2 * BAR)).unwrap();
    let mut events = rock(2, HAT);
    let wobble = [3i32, -2, 1, -3, 2, 0, -1];
    for (i, e) in events.iter_mut().enumerate().skip(2) {
        let shift = wobble[i % wobble.len()];
        e.0 = (e.0 as i32 + shift).max(0) as u32;
    }
    assert_eq!(one(&smf(events, 2 * BAR)).unwrap(), plain);
}

#[test]
fn swung_notes_snap_to_the_grid() {
    // Every odd step a third of a step late: far off the straight grid, but on the swung one.
    let mut events = vec![tempo(0, 100.0)];
    for step in 0..16 {
        let late = if step % 2 == 1 { STEP / 3 } else { 0 };
        events.push(note(step * STEP + late, 9, HAT, 80));
    }
    let groove = one(&smf(events, BAR)).unwrap();
    let steps: Vec<usize> = groove.bars[0].iter().map(|h| h.step).collect();
    assert_eq!(steps, (0..16).collect::<Vec<_>>());
}

#[test]
fn random_timing_is_off_grid() {
    let mut seed = 12345u32;
    let mut random = || {
        seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
        (seed >> 16) % STEP
    };
    let mut events = vec![tempo(0, 100.0)];
    for i in 0..64 {
        events.push(note(
            i * STEP + random(),
            9,
            [KICK, SNARE, HAT][i as usize % 3],
            90,
        ));
    }
    assert_eq!(one(&smf(events, 4 * BAR)), Err(Refusal::OffGrid));
}

#[test]
fn the_louder_of_two_notes_on_one_step_is_kept() {
    let events = vec![note(0, 9, SNARE, 40), note(2, 9, SNARE, 120)];
    let groove = one(&smf(events, BAR)).unwrap();
    assert_eq!(
        groove.bars[0],
        vec![BarHit {
            key: SNARE,
            step: 0,
            velocity: 120
        }]
    );
}

#[test]
fn a_note_just_before_the_end_wraps_to_the_start() {
    // A crash played a hair early for the next loop.
    let events = vec![note(0, 9, KICK, 100), note(BAR - 2, 9, 49, 100)];
    let groove = one(&smf(events, BAR)).unwrap();
    assert_eq!(groove.bars.len(), 1);
    assert!(groove.bars[0].contains(&BarHit {
        key: 49,
        step: 0,
        velocity: 100
    }));
}

#[test]
fn damaged_midi_is_refused() {
    // The reader forgives a cut-off track, but not a cut-off header.
    let mut bytes = smf(rock(1, HAT), BAR);
    bytes.truncate(10);
    assert_eq!(one(&bytes), Err(Refusal::NotMidi));
    // SMPTE timing: a negative frame rate in the header's division.
    let mut bytes = smf(rock(1, HAT), BAR);
    bytes[12] = 0xE7;
    bytes[13] = 40;
    assert_eq!(one(&bytes), Err(Refusal::NotMidi));
}

#[test]
fn garbage_is_not_grooves() {
    assert_eq!(
        import("page.html", b"<!doctype html><p>hi</p>"),
        Err(ImportError::NotGrooves)
    );
    assert_eq!(import("empty", b""), Err(ImportError::NotGrooves));
}

#[test]
fn too_large_input_is_refused() {
    let mut bytes = b"MThd".to_vec();
    bytes.resize(MAX_INPUT_BYTES + 1, 0);
    assert_eq!(import("big.mid", &bytes), Err(ImportError::TooLarge));
}

#[test]
fn zip_with_general_midi_folder_takes_only_that() {
    let file = smf(rock(1, HAT), BAR);
    let other = smf(rock(1, 63), BAR);
    let bytes = zip_of(&[
        ("Pack/GM/Rock/a.mid", &file),
        ("Pack/EZdrummer/Rock/a.mid", &other),
        ("Pack/About.txt", b"hello"),
    ]);
    let import = import("Pack.zip", &bytes).unwrap();
    assert_eq!(import.grooves.len(), 1);
    assert_eq!(import.grooves[0].name, "a");
    assert_eq!(import.grooves[0].folder, "Rock");
    assert_eq!(import.other_mappings_skipped, 1);
    assert!(import.refused.is_empty());
}

#[test]
fn zip_without_mappings_takes_everything_below_the_shared_top_folder() {
    let file = smf(rock(1, HAT), BAR);
    let bytes = zip_of(&[
        ("Beat Farm GM/Rock/Fast/a.mid", &file),
        ("Beat Farm GM/Pop/b.MIDI", &file),
        ("__MACOSX/Beat Farm GM/Pop/._b.MIDI", b"junk"),
        ("Beat Farm GM/.hidden.mid", b"junk"),
    ]);
    let import = import("Beat Farm.zip", &bytes).unwrap();
    let names: Vec<(&str, &str)> = import
        .grooves
        .iter()
        .map(|g| (g.folder.as_str(), g.name.as_str()))
        .collect();
    assert_eq!(names, [("Pop", "b"), ("Rock/Fast", "a")]);
    assert_eq!(import.other_mappings_skipped, 0);
    assert!(import.refused.is_empty());
}

#[test]
fn zip_reports_refused_files() {
    let good = smf(rock(1, HAT), BAR);
    let mut waltz = rock(1, HAT);
    waltz[1] = time_signature(0, 3, 2);
    let waltz = smf(waltz, BAR);
    let bytes = zip_of(&[
        ("good.mid", &good),
        ("readme.txt", b"not a groove"),
        ("fake.mid", b"<html>not midi</html>"),
        ("waltz.mid", &waltz),
    ]);
    let import = import("Mixed.zip", &bytes).unwrap();
    assert_eq!(import.grooves.len(), 1);
    assert_eq!(import.grooves[0].folder, "");
    assert_eq!(
        import.refused,
        [
            ("fake.mid".to_owned(), Refusal::NotMidi),
            ("waltz.mid".to_owned(), Refusal::NotFourFour)
        ]
    );
}

#[test]
fn zip_without_midi_files() {
    let bytes = zip_of(&[("readme.txt", b"hello")]);
    assert_eq!(import("x.zip", &bytes), Err(ImportError::NoMidiFiles));
}

#[test]
fn damaged_zip() {
    let mut bytes = zip_of(&[("a.mid", &smf(rock(1, HAT), BAR))]);
    bytes.truncate(bytes.len() / 2);
    assert_eq!(import("x.zip", &bytes), Err(ImportError::BrokenZip));
}

#[test]
fn zip_with_too_many_entries() {
    let names: Vec<String> = (0..=MAX_ZIP_ENTRIES).map(|i| format!("{i}.txt")).collect();
    let files: Vec<(&str, &[u8])> = names.iter().map(|n| (n.as_str(), &b""[..])).collect();
    assert_eq!(import("x.zip", &zip_of(&files)), Err(ImportError::TooLarge));
}

#[test]
fn too_large_midi_in_a_zip_is_refused() {
    let mut big = smf(rock(1, HAT), BAR);
    big.resize(MAX_MIDI_BYTES + 1, 0);
    let import = import("x.zip", &zip_of(&[("big.mid", &big)])).unwrap();
    assert_eq!(import.refused, [("big.mid".to_owned(), Refusal::NotMidi)]);
}

#[test]
fn general_midi_folder_names() {
    for name in [
        "GM",
        "gm",
        "General MIDI",
        "GM Mapping",
        "general_midi",
        "Beat Farm GM",
    ] {
        assert!(zipped::is_general_midi(name), "{name}");
    }
    for name in ["EZdrummer", "GM Rock", "Superior Drummer", "Gmail"] {
        assert!(!zipped::is_general_midi(name), "{name}");
    }
}

#[test]
fn pack_json_round_trips() {
    let groove = one(&smf(rock(2, HAT), 2 * BAR)).unwrap();
    let pack = Pack {
        name: "Beat Farm".into(),
        grooves: vec![
            groove.clone(),
            Groove {
                name: "Odd \"name\"".into(),
                folder: "Rock/Fast".into(),
                bpm: 87.0,
                ..groove
            },
        ],
    };
    let text = pack_to_json(&pack);
    assert!(text.starts_with(r#"{"format":1,"name":"Beat Farm","grooves":[{"#));
    assert_eq!(pack_from_json(&text), Ok(pack));
}

#[test]
fn pack_json_refuses_bad_data() {
    let groove = r#"{"name":"a","folder":"","bpm":100,"bars":[[[36,0,100]]]}"#;
    let ok = format!(r#"{{"format":1,"name":"p","grooves":[{groove}]}}"#);
    assert!(pack_from_json(&ok).is_ok());
    let newer = ok.replace(r#""format":1"#, r#""format":2"#);
    assert!(pack_from_json(&newer).unwrap_err().contains("format 2"));
    assert!(pack_from_json("not json").is_err());
    assert!(pack_from_json(r#"{"format":1}"#).is_err());
    for bad in ["[36,16,100]", "[36,0,0]", "[36,0,128]", "[128,0,1]"] {
        let text = ok.replace("[36,0,100]", bad);
        assert!(pack_from_json(&text).is_err(), "{bad}");
    }
    let no_bars = ok.replace("[[[36,0,100]]]", "[]");
    assert!(pack_from_json(&no_bars).is_err());
}
