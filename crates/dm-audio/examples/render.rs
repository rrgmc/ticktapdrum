//! Programs the rock beat of the `beat` example (pattern A, and a fill as pattern B), renders the
//! song A A A B once at 100 BPM without an audio device, and writes it to a WAV file. Prints the
//! length of the audio and how many times faster than real time it rendered.
//!
//! Usage: `cargo run -p dm-audio --release --example render -- <bank.sf2> <out.wav> [swing]`

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use dm_audio::{OfflineRender, write_wav};

const KICK: u8 = 36;
const SNARE: u8 = 38;
const CLOSED_HAT: u8 = 42;

const BPM: f32 = 100.0;
const STEPS: usize = 16;
const SAMPLE_RATE: u32 = 44_100;
const TAIL_SECONDS: f32 = 2.0;

/// Pattern slot A.
const A: usize = 0;
/// Pattern slot B.
const B: usize = 1;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let usage = "usage: render <bank.sf2> <out.wav> [swing]";
    let bank: PathBuf = args.next().ok_or(usage)?.into();
    let out: PathBuf = args.next().ok_or(usage)?.into();
    let swing: f32 = match args.next() {
        Some(swing) => swing.to_str().ok_or(usage)?.parse()?,
        None => 0.0,
    };

    let soundfont = dm_audio::load_soundfont(&bank)?;
    let mut offline = OfflineRender::new(soundfont, SAMPLE_RATE)?;

    offline.set_row_key(0, KICK);
    offline.set_row_key(1, SNARE);
    offline.set_row_key(2, CLOSED_HAT);

    // Pattern A: kick on 1 and the "and" of 3, snare on 2 and 4, hi-hat on every 8th note.
    for step in [0, 8, 10] {
        offline.set_step(A, 0, step, 120);
    }
    for step in [4, 12] {
        offline.set_step(A, 1, step, 110);
    }
    for step in (0..STEPS).step_by(2) {
        offline.set_step(A, 2, step, if step % 4 == 0 { 90 } else { 60 });
    }
    offline.set_length(A, STEPS);

    // Pattern B: two beats of the groove, then a snare roll on every 16th that gets louder, with
    // the kick on beats 3 and 4.
    offline.set_step(B, 0, 0, 120);
    offline.set_step(B, 1, 4, 110);
    for step in [0, 2, 4, 6] {
        offline.set_step(B, 2, step, if step % 4 == 0 { 90 } else { 60 });
    }
    for step in 8..STEPS {
        offline.set_step(B, 1, step, 60 + 8 * (step - 8) as u8);
    }
    offline.set_step(B, 0, 8, 110);
    offline.set_step(B, 0, 12, 110);
    offline.set_length(B, STEPS);

    offline.set_tempo(BPM);
    offline.set_swing(swing);
    offline.set_song(&[A, A, A, B]);

    let cancel = AtomicBool::new(false);
    let mut reports = 0;
    let start = Instant::now();
    let samples = offline.render_song(TAIL_SECONDS, &mut |_| reports += 1, &cancel)?;
    let elapsed = start.elapsed().as_secs_f64();
    write_wav(&out, &samples, SAMPLE_RATE)?;

    let seconds = (samples.len() / 2) as f64 / f64::from(SAMPLE_RATE);
    println!(
        "{}: {:.3} s of audio, {} bytes; rendered in {:.3} s, {:.0}x real time, {reports} progress reports",
        out.display(),
        seconds,
        std::fs::metadata(&out)?.len(),
        elapsed,
        seconds / elapsed
    );
    Ok(())
}
