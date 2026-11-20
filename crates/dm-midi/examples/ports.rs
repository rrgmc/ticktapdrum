//! Prints the MIDI outputs and inputs the system reports, as the interface would list them.
//!
//! `cargo run -p dm-midi --example ports`

use dm_midi::MidiOut;

fn main() {
    if !MidiOut::supported() {
        println!("MIDI is not supported on this platform.");
        return;
    }
    let (_producer, consumer) = rtrb::RingBuffer::new(1);
    let midi = MidiOut::start(consumer);
    print_list("Outputs", midi.outputs());
    print_list("Inputs", midi.inputs());
}

fn print_list(title: &str, names: Vec<String>) {
    println!("{title}:");
    if names.is_empty() {
        println!("  none");
    }
    for name in names {
        println!("  {name}");
    }
}
