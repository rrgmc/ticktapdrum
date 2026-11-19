//! Plays two bars of kick, snare and hi-hat at 100 BPM through the default output device.
//!
//! Usage: `cargo run -p dm-audio --example hit -- <bank.sf2>`

use std::path::PathBuf;
use std::thread::sleep;
use std::time::{Duration, Instant};

const KICK: u8 = 36;
const SNARE: u8 = 38;
const CLOSED_HAT: u8 = 42;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path: PathBuf = std::env::args_os()
        .nth(1)
        .ok_or("usage: hit <bank.sf2>")?
        .into();

    let loaded = Instant::now();
    let soundfont = dm_audio::load_soundfont(&path)?;
    println!("loaded the bank in {:.1?}", loaded.elapsed());

    let mut engine = dm_audio::Engine::start(soundfont)?;
    println!(
        "output: {} Hz, {} channels, device buffer {:?} frames",
        engine.sample_rate(),
        engine.channels(),
        engine.buffer_frames()
    );

    // 100 BPM in eighth notes: 0.3 s per step, 8 steps per bar.
    let step = Duration::from_secs_f64(60.0 / 100.0 / 2.0);
    let start = Instant::now();
    for i in 0..16u32 {
        let position = i % 8;
        if position == 0 || position == 4 {
            engine.hit(KICK, 120);
        }
        if position == 2 || position == 6 {
            engine.hit(SNARE, 110);
        }
        engine.hit(CLOSED_HAT, if position % 2 == 0 { 90 } else { 60 });

        // Sleep to the next step's absolute time so the sleeps do not drift.
        let next = start + step * (i + 1);
        sleep(next.saturating_duration_since(Instant::now()));
    }

    // Let the last hits ring out before the engine is dropped.
    sleep(Duration::from_millis(1500));
    Ok(())
}
