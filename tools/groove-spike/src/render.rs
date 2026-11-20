//! Renders a file twice with the same SoundFont: the original performance, and the imported
//! project through the app's own offline renderer. Also writes the project.

use std::path::Path;
use std::sync::atomic::AtomicBool;

use dm_audio::{OfflineRender, write_wav};
use dm_project::Change;
use rustysynth::{Synthesizer, SynthesizerSettings};

use crate::keymap::KeyMap;

const RATE: u32 = 44_100;
const TAIL_SECONDS: f32 = 2.0;
const PERCUSSION: i32 = 9;

pub fn run(
    midi: &Path,
    bank: &Path,
    out: &Path,
    map: KeyMap,
) -> Result<(), Box<dyn std::error::Error>> {
    std::fs::create_dir_all(out)?;
    let stem = midi.file_stem().unwrap().to_string_lossy().into_owned();
    let analysis = crate::analysis_of(midi, map)?;
    let built = crate::build::project(&analysis);
    println!("{}\n{}", analysis.report(), built.report());
    let json = dm_project::to_json(&built.project);
    // The app must load the file back unchanged.
    let loaded = dm_project::from_json(&json).map_err(|e| format!("{e:?}"))?;
    let tempo_only = loaded.tempo - built.project.tempo;
    let mut same = loaded.clone();
    same.tempo = built.project.tempo;
    println!(
        "  loads back {}; tempo differs by {tempo_only:.4}",
        if same == built.project {
            "unchanged"
        } else {
            "CHANGED"
        }
    );
    std::fs::write(out.join(format!("{stem}.drums")), json)?;

    let soundfont = dm_audio::load_soundfont(bank)?;

    let mut offline = OfflineRender::new(soundfont.clone(), RATE)?;
    offline.set_tempo(built.project.tempo);
    for change in built.project.full_sync() {
        apply(&mut offline, change);
    }
    let cancel = AtomicBool::new(false);
    let imported = offline.render_song(TAIL_SECONDS, &mut |_| {}, &cancel)?;
    write_wav(&out.join(format!("{stem}.imported.wav")), &imported, RATE)?;

    // The original: the same notes, keys mapped the same way, at their recorded times. Tempo
    // changes are ignored, as the import ignores them.
    let file = crate::read::parse(&std::fs::read(midi)?)?;
    let channels: std::collections::BTreeSet<u8> = file.notes.iter().map(|n| n.channel).collect();
    let drum_channel = if channels.contains(&9) || channels.len() != 1 {
        9
    } else {
        *channels.first().unwrap()
    };
    let seconds_per_tick = 60.0
        / (analysis.bpm
            * if crate::analysis::HALF_TIME.load(std::sync::atomic::Ordering::Relaxed) {
                2.0
            } else {
                1.0
            })
        / f64::from(file.ppq);
    // In steps and the analysed tempo, so it matches in half time too.
    let length = analysis.bars as f64 * 16.0 / 4.0 * 60.0 / analysis.bpm;
    let mut events: Vec<(usize, u8, u8)> = file
        .notes
        .iter()
        .filter(|n| n.channel == drum_channel)
        .filter_map(|n| {
            let t = n.tick as f64 * seconds_per_tick;
            let key = analysis.map.map(n.key)?;
            (t < length).then_some(((t * f64::from(RATE)) as usize, key, n.velocity))
        })
        .collect();
    events.sort_by_key(|e| e.0);

    let mut synth = Synthesizer::new(&soundfont, &SynthesizerSettings::new(RATE as i32))
        .map_err(|e| format!("{e:?}"))?;
    synth.process_midi_message(PERCUSSION, 0xC0, 0, 0);
    let total = ((length + f64::from(TAIL_SECONDS)) * f64::from(RATE)) as usize;
    let mut left = vec![0.0f32; total];
    let mut right = vec![0.0f32; total];
    let mut at = 0;
    for &(frame, key, velocity) in &events {
        let frame = frame.min(total);
        if frame > at {
            synth.render(&mut left[at..frame], &mut right[at..frame]);
            at = frame;
        }
        synth.note_on(PERCUSSION, i32::from(key), i32::from(velocity));
    }
    synth.render(&mut left[at..], &mut right[at..]);
    let original: Vec<f32> = left
        .iter()
        .zip(&right)
        .flat_map(|(l, r)| [*l, *r])
        .collect();
    write_wav(&out.join(format!("{stem}.original.wav")), &original, RATE)?;
    println!(
        "  wrote {stem}.drums, {stem}.original.wav ({:.1} s), {stem}.imported.wav ({:.1} s)",
        original.len() as f64 / 2.0 / f64::from(RATE),
        imported.len() as f64 / 2.0 / f64::from(RATE)
    );
    Ok(())
}

/// As the app's export applies changes to the offline renderer.
fn apply(offline: &mut OfflineRender, change: Change) {
    match change {
        Change::Step {
            slot,
            row,
            step,
            velocity,
        } => offline.set_step(slot, row, step, velocity),
        Change::Length { slot, steps } => offline.set_length(slot, steps),
        Change::Song(order) => offline.set_song(&order),
        Change::RowKey { row, key } => offline.set_row_key(row, key),
        Change::RowGain { row, percent } => offline.set_row_gain(row, f32::from(percent) / 100.0),
        Change::ClearSlot { slot } => offline.clear_pattern(slot),
        Change::Kit(program) => offline.set_kit(program),
        Change::Swing(thousandths) => offline.set_swing(f32::from(thousandths) / 1000.0),
    }
}
