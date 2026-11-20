//! Listens to a MIDI input and prints each drum hit, for a manual test against a real keyboard
//! or drum pads. Needs no audio engine.
//!
//! `cargo run -p dm-midi --example listen -- "<input name>" ["<output name>"]`
//!
//! With an output name as well, the output is opened too. Give the same name twice to see the
//! shared connection, whose hits are not to be sounded. Each line shows the key, the velocity,
//! whether the app would sound the hit, and how long before the line was printed the hit
//! happened, by the input's timestamp. Stop with Ctrl+C.

use std::thread;
use std::time::{Duration, Instant};

use dm_audio::LIVE_QUEUE_CAPACITY;
use dm_midi::{InputChoice, MidiOut, MidiStatus};

fn main() {
    if !MidiOut::supported() {
        println!("MIDI is not supported on this platform.");
        return;
    }
    let mut args = std::env::args().skip(1);
    // The engine's MIDI queue; nothing is pushed into it here.
    let (_events, events) = rtrb::RingBuffer::new(1);
    let mut midi = MidiOut::start(events);
    let Some(input) = args.next() else {
        eprintln!("usage: listen <input name> [<output name>]");
        eprintln!("inputs:");
        for name in midi.inputs() {
            eprintln!("  {name}");
        }
        std::process::exit(2);
    };
    let output = args.next();

    let (hits, mut heard) = rtrb::RingBuffer::new(LIVE_QUEUE_CAPACITY);
    midi.listen(hits);
    midi.select(output);
    midi.select_input(InputChoice::Named(input.clone()));

    let mut input_status = MidiStatus::Off;
    let mut output_status = MidiStatus::Off;
    loop {
        let status = midi.input_status();
        if status != input_status {
            println!("input {input:?}: {status:?}");
            input_status = status;
        }
        let status = midi.status();
        if status != output_status {
            println!("output: {status:?}");
            output_status = status;
        }
        while let Ok(hit) = heard.pop() {
            let ago = Instant::now().saturating_duration_since(hit.at);
            println!(
                "key {:3}  velocity {:3}  sound {:5}  {:6.1} ms ago",
                hit.key,
                hit.velocity,
                hit.sound,
                ago.as_secs_f64() * 1000.0
            );
        }
        thread::sleep(Duration::from_millis(5));
    }
}
