//! Programs a rock beat as pattern A and a fill as pattern B, then plays the song A A A B once at
//! 100 BPM and exits when it has ended.
//!
//! Usage: `cargo run -p dm-audio --example beat -- <bank.sf2>`

use std::path::PathBuf;
use std::thread::sleep;
use std::time::{Duration, Instant};

const KICK: u8 = 36;
const SNARE: u8 = 38;
const CLOSED_HAT: u8 = 42;

const BPM: f32 = 100.0;
const STEPS: usize = 16;

/// Pattern slot A.
const A: usize = 0;
/// Pattern slot B.
const B: usize = 1;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path: PathBuf = std::env::args_os()
        .nth(1)
        .ok_or("usage: beat <bank.sf2>")?
        .into();

    let soundfont = dm_audio::load_soundfont(&path)?;
    let mut engine = dm_audio::Engine::start(soundfont)?;

    engine.set_row_key(0, KICK);
    engine.set_row_key(1, SNARE);
    engine.set_row_key(2, CLOSED_HAT);

    // Pattern A: kick on 1 and the "and" of 3, snare on 2 and 4, hi-hat on every 8th note.
    for step in [0, 8, 10] {
        engine.set_step(A, 0, step, 120);
    }
    for step in [4, 12] {
        engine.set_step(A, 1, step, 110);
    }
    for step in (0..STEPS).step_by(2) {
        engine.set_step(A, 2, step, if step % 4 == 0 { 90 } else { 60 });
    }
    engine.set_length(A, STEPS);

    // Pattern B: two beats of the groove, then a snare roll on every 16th that gets louder, with
    // the kick on beats 3 and 4.
    engine.set_step(B, 0, 0, 120);
    engine.set_step(B, 1, 4, 110);
    engine.set_step(B, 2, 0, 90);
    engine.set_step(B, 2, 2, 60);
    engine.set_step(B, 2, 4, 90);
    engine.set_step(B, 2, 6, 60);
    for step in 8..STEPS {
        engine.set_step(B, 1, step, 60 + 8 * (step - 8) as u8);
    }
    engine.set_step(B, 0, 8, 110);
    engine.set_step(B, 0, 12, 110);
    engine.set_length(B, STEPS);

    engine.set_tempo(BPM);
    engine.set_song(&[A, A, A, B]);
    engine.set_song_loop(false);
    engine.play_song(0);

    let start = Instant::now();
    let mut shown = None;
    while engine.is_playing() {
        let playhead = engine.playhead();
        if playhead != shown && playhead.is_some_and(|p| p.step % 4 == 0) {
            println!(
                "{:5.2} s: {:?} (latency {} frames)",
                start.elapsed().as_secs_f64(),
                playhead,
                engine.output_latency_frames()
            );
        }
        shown = playhead;
        sleep(Duration::from_millis(5));
    }
    println!(
        "{:5.2} s: song ended; playhead {:?}",
        start.elapsed().as_secs_f64(),
        engine.playhead()
    );

    // Let the last hits ring out before the engine is dropped.
    sleep(Duration::from_millis(1500));
    println!("done; playhead {:?}", engine.playhead());
    Ok(())
}
