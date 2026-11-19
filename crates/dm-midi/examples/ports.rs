//! Prints the MIDI outputs the system reports, as the interface would list them.
//!
//! `cargo run -p dm-midi --example ports`

use dm_midi::MidiOut;

fn main() {
    if !MidiOut::supported() {
        println!("MIDI out is not supported on this platform.");
        return;
    }
    let (_producer, consumer) = rtrb::RingBuffer::new(1);
    let midi = MidiOut::start(consumer);
    let outputs = midi.outputs();
    if outputs.is_empty() {
        println!("No MIDI outputs.");
    }
    for name in outputs {
        println!("{name}");
    }
}
