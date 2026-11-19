//! The sender thread and its connection to the output, through `midir`. Not built on Android.

use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use dm_audio::MidiEvents;
use midir::{MidiOutput, MidiOutputConnection};

use crate::schedule::{Pending, Sounding, output_list, should_send};
use crate::{MidiStatus, SendHook};

/// The name the app gives itself as a MIDI client.
const CLIENT_NAME: &str = "TickTapDrum";

/// How long the thread sleeps at most while an output is open, so that a message that arrives
/// in the queue goes out within about this long of its due time.
const CONNECTED_NAP: Duration = Duration::from_millis(2);

/// How long the thread sleeps at most while no output is open. It only discards messages and
/// waits for a choice then.
const IDLE_NAP: Duration = Duration::from_millis(10);

/// How often the thread checks that the chosen output is still listed, or looks for it again.
const PORT_CHECK: Duration = Duration::from_secs(1);

/// The virtual port, where the platform can create one.
#[cfg(any(target_os = "macos", target_os = "linux"))]
const VIRTUAL: Option<&str> = Some(crate::VIRTUAL_PORT);
#[cfg(not(any(target_os = "macos", target_os = "linux")))]
const VIRTUAL: Option<&str> = None;

/// A request from the interface to the thread.
enum Control {
    Select(Option<String>),
    Stop,
}

/// The interface's handle on the sender thread.
pub(crate) struct Handle {
    control: Sender<Control>,
    status: Arc<Mutex<MidiStatus>>,
    thread: Option<JoinHandle<()>>,
    /// The output last passed to [`Handle::select`].
    chosen: Option<String>,
}

impl Handle {
    pub(crate) fn start(events: MidiEvents, hook: Option<SendHook>) -> Self {
        let (control, requests) = mpsc::channel();
        let status = Arc::new(Mutex::new(MidiStatus::Off));
        let worker = Worker {
            events,
            requests,
            status: Arc::clone(&status),
            hook,
            lister: None,
            wanted: None,
            connection: None,
            is_virtual: false,
            pending: Pending::default(),
            sounding: Sounding::default(),
            next_check: Instant::now() + PORT_CHECK,
        };
        let thread = thread::Builder::new()
            .name("midi-out".to_owned())
            .spawn(move || worker.run());
        let thread = match thread {
            Ok(thread) => Some(thread),
            Err(e) => {
                log::error!("MIDI out: could not start the sender thread: {e}");
                *status.lock().unwrap_or_else(PoisonError::into_inner) =
                    MidiStatus::Failed(e.to_string());
                None
            }
        };
        Handle {
            control,
            status,
            thread,
            chosen: None,
        }
    }

    pub(crate) fn select(&mut self, name: Option<String>) {
        if name == self.chosen {
            return;
        }
        self.chosen = name.clone();
        // The old output's status must not be read as the new one's while the thread has not yet
        // acted on the choice; otherwise the interface would say "sending" too early. The thread
        // overwrites this with the outcome.
        *self.status.lock().unwrap_or_else(PoisonError::into_inner) = MidiStatus::Off;
        // A send fails only when the thread has ended, and then there is nothing to tell it.
        let _ = self.control.send(Control::Select(name));
    }

    pub(crate) fn status(&self) -> MidiStatus {
        self.status
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        let _ = self.control.send(Control::Stop);
        if let Some(thread) = self.thread.take()
            && thread.join().is_err()
        {
            log::error!("MIDI out: the sender thread panicked");
        }
    }
}

/// The output names the system reports now, as the interface lists them.
pub(crate) fn outputs() -> Vec<String> {
    let names = match MidiOutput::new(CLIENT_NAME) {
        Ok(output) => port_names(&output),
        Err(e) => {
            log::warn!("MIDI out: cannot list the outputs: {e}");
            Vec::new()
        }
    };
    output_list(names, VIRTUAL)
}

fn port_names(output: &MidiOutput) -> Vec<String> {
    output
        .ports()
        .iter()
        .filter_map(|port| output.port_name(port).ok())
        .collect()
}

/// The thread's state. It owns the queue and the connection.
struct Worker {
    events: MidiEvents,
    requests: Receiver<Control>,
    status: Arc<Mutex<MidiStatus>>,
    hook: Option<SendHook>,
    /// A client kept for the once-a-second check of the port list. Connecting consumes a
    /// `MidiOutput`, so connections use clients of their own.
    lister: Option<MidiOutput>,
    /// The output the interface chose.
    wanted: Option<String>,
    connection: Option<MidiOutputConnection>,
    /// Whether `connection` is the app's own virtual port, which is never in the port list.
    is_virtual: bool,
    pending: Pending,
    sounding: Sounding,
    /// When to next check the port list.
    next_check: Instant,
}

impl Worker {
    fn run(mut self) {
        loop {
            loop {
                match self.requests.try_recv() {
                    Ok(Control::Select(name)) => self.select(name),
                    Ok(Control::Stop) | Err(TryRecvError::Disconnected) => {
                        self.disconnect();
                        return;
                    }
                    Err(TryRecvError::Empty) => break,
                }
            }

            if Instant::now() >= self.next_check {
                self.check_port();
            }
            self.take_events();
            self.send_due();
            self.nap();
        }
    }

    /// Sleeps until the next message is due, the next port check, or the nap's end, whichever
    /// comes first.
    fn nap(&self) {
        let now = Instant::now();
        let nap = if self.connection.is_some() {
            CONNECTED_NAP
        } else {
            IDLE_NAP
        };
        let mut until = (now + nap).min(self.next_check);
        if let Some(due) = self.pending.next_due() {
            until = until.min(due);
        }
        if until > now {
            thread::sleep(until - now);
        }
    }

    fn set_status(&self, status: MidiStatus) {
        let mut current = self.status.lock().unwrap_or_else(PoisonError::into_inner);
        if *current != status {
            match &status {
                MidiStatus::Off => log::info!("MIDI out: off"),
                MidiStatus::Connected => log::info!("MIDI out: connected to {:?}", self.wanted),
                MidiStatus::Missing => log::info!("MIDI out: {:?} is not there", self.wanted),
                MidiStatus::Failed(reason) => {
                    log::warn!("MIDI out: cannot open {:?}: {reason}", self.wanted)
                }
            }
            *current = status;
        }
    }

    fn select(&mut self, name: Option<String>) {
        if name == self.wanted {
            return;
        }
        self.disconnect();
        self.wanted = name;
        if self.wanted.is_some() {
            self.connect();
        } else {
            self.set_status(MidiStatus::Off);
        }
    }

    /// Tries to open the chosen output, and sets the status to match.
    fn connect(&mut self) {
        self.next_check = Instant::now() + PORT_CHECK;
        let Some(name) = self.wanted.clone() else {
            return;
        };
        let result = if VIRTUAL == Some(name.as_str()) {
            self.is_virtual = true;
            open_virtual()
        } else {
            self.is_virtual = false;
            open_port(&name)
        };
        match result {
            Ok(connection) => {
                self.connection = Some(connection);
                self.pending.clear();
                self.sounding = Sounding::default();
                self.set_status(MidiStatus::Connected);
            }
            Err(status) => self.set_status(status),
        }
    }

    /// Sends the silencing messages and closes the connection, if one is open.
    fn disconnect(&mut self) {
        if let Some(mut connection) = self.connection.take() {
            for message in self.sounding.silence() {
                if let Err(e) = connection.send(&message) {
                    log::warn!("MIDI out: could not silence the output: {e}");
                    break;
                }
            }
            connection.close();
        }
        self.pending.clear();
        self.sounding = Sounding::default();
    }

    /// Drops the connection without silencing it, because the output has gone.
    fn lose_connection(&mut self) {
        self.connection = None;
        self.pending.clear();
        self.sounding = Sounding::default();
        self.next_check = Instant::now() + PORT_CHECK;
        self.set_status(MidiStatus::Missing);
    }

    /// Once a second: checks that the open output is still listed, or tries again to open the
    /// chosen one.
    fn check_port(&mut self) {
        self.next_check = Instant::now() + PORT_CHECK;
        let Some(name) = self.wanted.clone() else {
            return;
        };
        if self.connection.is_none() {
            self.connect();
        } else if !self.is_virtual && !self.is_listed(&name) {
            self.lose_connection();
        }
    }

    fn is_listed(&mut self, name: &str) -> bool {
        if self.lister.is_none() {
            self.lister = MidiOutput::new(CLIENT_NAME).ok();
        }
        match &self.lister {
            Some(lister) => lister
                .ports()
                .iter()
                .any(|port| lister.port_name(port).is_ok_and(|n| n == name)),
            // Without a client the list cannot be read; keep the connection rather than guess.
            None => true,
        }
    }

    /// Moves the messages from the engine's queue into the pending list, or discards them while
    /// no output is open.
    fn take_events(&mut self) {
        while let Ok(event) = self.events.pop() {
            if self.connection.is_some() && !self.pending.push(event) {
                log::warn!("MIDI out: too many messages waiting; dropped one");
            }
        }
    }

    /// Sends every pending message that is due, dropping those that are too late.
    fn send_due(&mut self) {
        let Some(connection) = self.connection.as_mut() else {
            self.pending.clear();
            return;
        };
        let now = Instant::now();
        while let Some(event) = self.pending.pop_due(now) {
            if !should_send(&event, now, &self.sounding) {
                log::debug!("MIDI out: dropped a message that was too late");
                continue;
            }
            let message = event.message();
            let sent_at = Instant::now();
            match connection.send(message) {
                Ok(()) => {
                    self.sounding.track(message);
                    if let Some(hook) = self.hook.as_mut() {
                        hook(&event, sent_at);
                    }
                }
                Err(e) => {
                    log::warn!("MIDI out: send failed: {e}");
                    self.lose_connection();
                    return;
                }
            }
        }
    }
}

/// Opens the output port with this name.
fn open_port(name: &str) -> Result<MidiOutputConnection, MidiStatus> {
    let output = MidiOutput::new(CLIENT_NAME).map_err(|e| MidiStatus::Failed(e.to_string()))?;
    let port = output
        .ports()
        .into_iter()
        .find(|port| output.port_name(port).is_ok_and(|n| n == name))
        .ok_or(MidiStatus::Missing)?;
    output
        .connect(&port, CLIENT_NAME)
        .map_err(|e| MidiStatus::Failed(e.to_string()))
}

/// Creates the app's virtual output port.
#[cfg(any(target_os = "macos", target_os = "linux"))]
fn open_virtual() -> Result<MidiOutputConnection, MidiStatus> {
    use midir::os::unix::VirtualOutput;

    let output = MidiOutput::new(CLIENT_NAME).map_err(|e| MidiStatus::Failed(e.to_string()))?;
    output
        .create_virtual(crate::VIRTUAL_PORT)
        .map_err(|e| MidiStatus::Failed(e.to_string()))
}

/// No virtual port here; [`VIRTUAL`] is `None`, so this is never called.
#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn open_virtual() -> Result<MidiOutputConnection, MidiStatus> {
    Err(MidiStatus::Missing)
}
