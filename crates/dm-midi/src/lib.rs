//! Sends TickTapDrum's notes to a MIDI output: a hardware drum module or synthesizer, or
//! another program. Also listens to a MIDI input for drum hits played on it.
//!
//! [`MidiOut::start`] takes the receiving end of the engine's MIDI queue and runs a thread that
//! sends each message when it falls due. The interface picks the output by name with
//! [`MidiOut::select`] and reads [`MidiOut::status`] each frame. While the status is
//! [`MidiStatus::Connected`], the interface keeps the engine's MIDI out on (which silences the
//! internal synthesizer); otherwise off.
//!
//! [`MidiOut::send_style`] writes a style file into a Yamaha arranger keyboard through the same
//! output, with the input of the same name for the keyboard's replies.
//!
//! For MIDI in, [`MidiOut::listen`] takes the sending end of the engine's live hit queue, and the
//! interface picks the input with [`MidiOut::select_input`]. By default the input is the one named
//! like the output. Each note-on of a General MIDI drum key on the input's first channel goes into
//! the queue as a [`dm_audio::LiveHit`], timed by the input's own timestamp. An input with the
//! output's name is opened once and shared with Send to keyboard and the switch-off check, since
//! Windows lets only one connection hold a port. [`MidiOut::input_status`] says how it goes.
//!
//! See "Milestone 9: MIDI out", "Export as a keyboard style" and "Milestone 14: MIDI in" in
//! `docs/DESIGN.md`.

use std::time::Instant;

use dm_audio::{LiveHits, MidiEvent, MidiEvents};

#[cfg_attr(target_os = "android", allow(dead_code))]
mod input;
#[cfg_attr(target_os = "android", allow(dead_code))]
mod liveness;
#[cfg_attr(target_os = "android", allow(dead_code))]
mod schedule;
#[cfg(not(target_os = "android"))]
mod sender;
#[doc(hidden)]
pub mod transfer;

/// The name of the virtual output port the app creates where the system allows it (macOS and
/// Linux), which other programs see as an input.
pub const VIRTUAL_PORT: &str = "TickTapDrum";

/// What the MIDI thread is doing with the chosen output, or with the chosen input.
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

/// Which MIDI input the drum hits come from.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum InputChoice {
    /// The input with the same name as the chosen output. A keyboard has one of each, so this
    /// needs no second choice. With no output chosen, nothing is listened to.
    #[default]
    SameAsOutput,
    /// The input with this name, for example drum pads that have no output.
    Named(String),
    /// No input: hits are not listened to.
    None,
}

/// Which kind of keyboard a style goes to. Each brand has its own file transfer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyboardTarget {
    /// A Yamaha arranger: PSR-SX, Genos and the like. The style goes into the User drive's
    /// `STYLE` folder.
    Yamaha,
}

/// What a style transfer started with [`MidiOut::send_style`] is doing.
#[derive(Debug, Clone, PartialEq)]
pub enum TransferStatus {
    /// No transfer has been asked for.
    Idle,
    /// The transfer is under way: the share of the file sent so far, from 0 to 1.
    Running(f32),
    /// The file is saved on the keyboard under this name. Stays until the next transfer.
    Done(String),
    /// The transfer failed. Stays until the next transfer.
    Failed(TransferError),
}

/// Why a style transfer failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransferError {
    /// No output is connected, or it changed or went away during the transfer.
    NotConnected,
    /// The output has no input of the same name, so the keyboard's replies cannot be heard.
    NoInput,
    /// The keyboard did not answer.
    NoAnswer,
    /// Something answered, but it does not take files the way the target's brand does. The text
    /// is the model it named, or its reply in hex if it named none.
    UnknownKeyboard(String),
    /// The keyboard refused a step, or a file of the same name could not be replaced. The text
    /// is for the log.
    Refused(String),
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

    /// Gives the thread the queue to put drum hits into, the sending end of the engine's live
    /// hit queue. Until this is called, notes on the input are ignored. A second call replaces
    /// the first queue.
    pub fn listen(&mut self, hits: LiveHits) {
        #[cfg(not(target_os = "android"))]
        self.handle.listen(hits);
        #[cfg(target_os = "android")]
        let _ = hits;
    }

    /// The inputs there are now, by name, sorted. Enumerates afresh on each call, so call it when
    /// a list opens, not every frame. Leaves out the app's own virtual port.
    pub fn inputs(&self) -> Vec<String> {
        #[cfg(not(target_os = "android"))]
        {
            sender::inputs()
        }
        #[cfg(target_os = "android")]
        {
            Vec::new()
        }
    }

    /// Chooses the input the drum hits come from. Never blocks: the thread opens it. A different
    /// choice, even one that names the same port, makes the input take a channel afresh.
    pub fn select_input(&mut self, choice: InputChoice) {
        #[cfg(not(target_os = "android"))]
        self.handle.select_input(choice);
        #[cfg(target_os = "android")]
        let _ = choice;
    }

    /// What the thread is doing with the chosen input: [`MidiStatus::Off`] when no input applies
    /// ([`InputChoice::None`], or [`InputChoice::SameAsOutput`] with no output chosen),
    /// [`MidiStatus::Connected`] while it is open. Cheap; call it every frame.
    pub fn input_status(&self) -> MidiStatus {
        #[cfg(not(target_os = "android"))]
        {
            self.handle.input_status()
        }
        #[cfg(target_os = "android")]
        {
            MidiStatus::Off
        }
    }

    /// Starts writing a style file into the keyboard where `target` keeps its styles (for
    /// Yamaha, the User drive's `STYLE` folder), through the chosen output and the input of the
    /// same name. Replaces a file of the same name. Never blocks: the thread does the work, and
    /// [`MidiOut::transfer`] tells how it goes.
    ///
    /// While the transfer runs, the thread silences the output and discards the engine's
    /// messages. Asking again while one runs lets the first finish, then starts the second.
    pub fn send_style(&mut self, target: KeyboardTarget, file_name: String, data: Vec<u8>) {
        #[cfg(not(target_os = "android"))]
        self.handle.send_style(target, file_name, data);
        #[cfg(target_os = "android")]
        let _ = (target, file_name, data);
    }

    /// What the style transfer is doing. Cheap; call it every frame.
    pub fn transfer(&self) -> TransferStatus {
        #[cfg(not(target_os = "android"))]
        {
            self.handle.transfer()
        }
        #[cfg(target_os = "android")]
        {
            TransferStatus::Idle
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

    /// Waits up to a second for the input status to satisfy `ok`.
    fn wait_for_input(midi: &MidiOut, ok: impl Fn(&MidiStatus) -> bool) -> MidiStatus {
        let deadline = Instant::now() + Duration::from_secs(1);
        loop {
            let status = midi.input_status();
            if ok(&status) || Instant::now() > deadline {
                return status;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn the_input_status_follows_the_choices() {
        let (_producer, consumer) = rtrb::RingBuffer::new(16);
        let mut midi = MidiOut::start(consumer);
        let (hits, _heard) = rtrb::RingBuffer::new(dm_audio::LIVE_QUEUE_CAPACITY);
        midi.listen(hits);
        let absent = |s: &MidiStatus| matches!(s, MidiStatus::Missing | MidiStatus::Failed(_));

        // "Same as output" with no output listens to nothing.
        std::thread::sleep(Duration::from_millis(20));
        assert_eq!(midi.input_status(), MidiStatus::Off);

        // With an output chosen, it looks for the input of that name.
        midi.select(Some("No such device, surely".to_owned()));
        let status = wait_for_input(&midi, absent);
        assert!(absent(&status), "{status:?}");

        midi.select_input(InputChoice::None);
        assert_eq!(
            wait_for_input(&midi, |s| *s == MidiStatus::Off),
            MidiStatus::Off
        );

        midi.select(None);
        midi.select_input(InputChoice::Named("No such input, surely".to_owned()));
        let status = wait_for_input(&midi, absent);
        assert!(absent(&status), "{status:?}");

        midi.select_input(InputChoice::SameAsOutput);
        assert_eq!(
            wait_for_input(&midi, |s| *s == MidiStatus::Off),
            MidiStatus::Off
        );
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

        // A style cannot go to an output that is not there.
        assert_eq!(midi.transfer(), TransferStatus::Idle);
        midi.send_style(KeyboardTarget::Yamaha, "A.sty".to_owned(), vec![1, 2, 3]);
        let deadline = Instant::now() + Duration::from_secs(1);
        while matches!(midi.transfer(), TransferStatus::Running(_)) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(
            midi.transfer(),
            TransferStatus::Failed(TransferError::NotConnected)
        );

        midi.select(None);
        assert_eq!(wait_for(&midi, |s| *s == MidiStatus::Off), MidiStatus::Off);
        drop(midi);
        assert!(producer.is_abandoned(), "dropping should end the thread");
    }
}
