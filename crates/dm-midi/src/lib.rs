//! Sends TickTapDrum's notes to a MIDI output: a hardware drum module or synthesizer, or
//! another program.
//!
//! [`MidiOut::start`] takes the receiving end of the engine's MIDI queue and runs a thread that
//! sends each message when it falls due. The interface picks the output by name with
//! [`MidiOut::select`] and reads [`MidiOut::status`] each frame. While the status is
//! [`MidiStatus::Connected`], the interface keeps the engine's MIDI out on (which silences the
//! internal synthesizer); otherwise off.
//!
//! See "Milestone 9: MIDI out" in `docs/DESIGN.md`.

use std::time::Instant;

use dm_audio::{MidiEvent, MidiEvents};

#[cfg_attr(target_os = "android", allow(dead_code))]
mod schedule;
#[cfg(not(target_os = "android"))]
mod sender;

/// The name of the virtual output port the app creates where the system allows it (macOS and
/// Linux), which other programs see as an input.
pub const VIRTUAL_PORT: &str = "TickTapDrum";

/// What the MIDI thread is doing with the chosen output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MidiStatus {
    /// No output is chosen.
    Off,
    /// The chosen output is open and messages go to it.
    Connected,
    /// The chosen output is not there (unplugged, or the other program closed). The thread keeps
    /// looking for it and reconnects when it returns.
    Missing,
    /// The chosen output is there but would not open, for example because another program holds
    /// it. The text is the system's reason, for the log. The thread keeps trying.
    Failed(String),
}

/// The MIDI sender: a thread that owns the connection.
pub struct MidiOut {
    #[cfg(not(target_os = "android"))]
    handle: sender::Handle,
}

/// Called on the sender thread after each message goes out, with the message and the moment just
/// before it was handed to the system. For measuring timing in the examples; the app has no use
/// for it.
#[doc(hidden)]
pub type SendHook = Box<dyn FnMut(&MidiEvent, Instant) + Send>;

impl MidiOut {
    /// Whether this platform can send MIDI at all. False on Android.
    pub const fn supported() -> bool {
        cfg!(not(target_os = "android"))
    }

    /// Starts the sender thread. It sends nothing until an output is selected.
    pub fn start(events: MidiEvents) -> Self {
        Self::start_with_hook(events, None)
    }

    /// [`MidiOut::start`], with a hook that sees each message as it goes out. For the timing
    /// measurement in the `send` example; not part of the app's interface.
    #[doc(hidden)]
    pub fn start_with_hook(events: MidiEvents, hook: Option<SendHook>) -> Self {
        #[cfg(not(target_os = "android"))]
        {
            MidiOut {
                handle: sender::Handle::start(events, hook),
            }
        }
        // There is no MIDI here: the queue is dropped, and the engine's pushes into it fail
        // harmlessly.
        #[cfg(target_os = "android")]
        {
            let _ = (events, hook);
            MidiOut {}
        }
    }

    /// The outputs there are now, by name, sorted, including [`VIRTUAL_PORT`] where the system
    /// allows one. Enumerates afresh on each call, so call it when a list opens, not every frame.
    pub fn outputs(&self) -> Vec<String> {
        #[cfg(not(target_os = "android"))]
        {
            sender::outputs()
        }
        #[cfg(target_os = "android")]
        {
            Vec::new()
        }
    }

    /// Chooses the output by name, or `None` for no output. Never blocks: the thread opens it.
    pub fn select(&mut self, name: Option<String>) {
        #[cfg(not(target_os = "android"))]
        self.handle.select(name);
        #[cfg(target_os = "android")]
        let _ = name;
    }

    /// What the thread is doing with the chosen output. Cheap; call it every frame.
    pub fn status(&self) -> MidiStatus {
        #[cfg(not(target_os = "android"))]
        {
            self.handle.status()
        }
        #[cfg(target_os = "android")]
        {
            MidiStatus::Off
        }
    }
}

#[cfg(all(test, not(target_os = "android")))]
mod tests {
    use std::time::Duration;

    use super::*;

    /// Waits up to a second for the status to satisfy `ok`.
    fn wait_for(midi: &MidiOut, ok: impl Fn(&MidiStatus) -> bool) -> MidiStatus {
        let deadline = Instant::now() + Duration::from_secs(1);
        loop {
            let status = midi.status();
            if ok(&status) || Instant::now() > deadline {
                return status;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn an_absent_output_is_missing_and_queued_messages_are_discarded() {
        let (mut producer, consumer) = rtrb::RingBuffer::new(16);
        let mut midi = MidiOut::start(consumer);
        assert_eq!(midi.status(), MidiStatus::Off);

        midi.select(Some("No such output, surely".to_owned()));
        // Without a MIDI system at all (a bare Linux machine), opening fails instead.
        let status = wait_for(&midi, |s| *s != MidiStatus::Off);
        assert!(
            matches!(status, MidiStatus::Missing | MidiStatus::Failed(_)),
            "{status:?}"
        );

        for _ in 0..16 {
            let event = MidiEvent {
                at: Instant::now(),
                bytes: [0x99, 36, 100],
                len: 3,
            };
            producer.push(event).unwrap();
        }
        let deadline = Instant::now() + Duration::from_secs(1);
        while producer.slots() < 16 && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(producer.slots(), 16, "the thread should empty the queue");

        midi.select(None);
        assert_eq!(wait_for(&midi, |s| *s == MidiStatus::Off), MidiStatus::Off);
        drop(midi);
        assert!(producer.is_abandoned(), "dropping should end the thread");
    }
}
