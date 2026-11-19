//! MIDI out: the messages the audio thread hands to a MIDI sender, each with the moment it is due.
//!
//! The audio thread never calls a MIDI API, because those calls can block. It pushes each message
//! into a queue instead, and another thread (in the `dm-midi` crate) sends it when it falls due.

use std::time::Instant;

/// How many MIDI messages the queue holds.
pub const MIDI_QUEUE_CAPACITY: usize = 4096;

/// One MIDI message from the audio thread.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MidiEvent {
    /// When the message is due: the moment the matching internal sound would be heard.
    pub at: Instant,
    /// The message's bytes. Only the first `len` are used.
    pub bytes: [u8; 3],
    /// How many of `bytes` the message uses, 1..=3.
    pub len: u8,
}

impl MidiEvent {
    /// The message, ready to send.
    pub fn message(&self) -> &[u8] {
        &self.bytes[..usize::from(self.len.min(3))]
    }
}

/// The receiving end of the MIDI queue.
pub type MidiEvents = rtrb::Consumer<MidiEvent>;
