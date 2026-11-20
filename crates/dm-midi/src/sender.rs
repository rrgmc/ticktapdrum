//! The sender thread and its connections to the output, and to the input of the same name,
//! through `midir`. Not built on Android.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, TryRecvError};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use dm_audio::MidiEvents;
use midir::{Ignore, MidiInput, MidiInputConnection, MidiOutput, MidiOutputConnection};

use crate::liveness::{Action, Presence, Watch};
use crate::schedule::{Pending, Sounding, output_list, should_send};
use crate::transfer::{self, Fault, Link, REPLY_TIMEOUT, show};
use crate::{KeyboardTarget, MidiStatus, SendHook, TransferError, TransferStatus};

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

/// How long the thread waits for a reply at a time during a transfer before it looks at the
/// interface's requests and the engine's queue.
const TRANSFER_POLL: Duration = Duration::from_millis(10);

/// The status byte of Active Sensing.
const ACTIVE_SENSING: u8 = 0xFE;

/// The virtual port, where the platform can create one.
#[cfg(any(target_os = "macos", target_os = "linux"))]
const VIRTUAL: Option<&str> = Some(crate::VIRTUAL_PORT);
#[cfg(not(any(target_os = "macos", target_os = "linux")))]
const VIRTUAL: Option<&str> = None;

/// A request from the interface to the thread.
enum Control {
    Select(Option<String>),
    SendStyle(Job),
    Stop,
}

/// A style to write into the keyboard.
struct Job {
    /// Which request this is; only the latest one may set the transfer status.
    number: u64,
    target: KeyboardTarget,
    name: String,
    data: Vec<u8>,
}

/// The transfer status with the number of the request it belongs to.
type Transfer = Arc<Mutex<(u64, TransferStatus)>>;

/// The interface's handle on the sender thread.
pub(crate) struct Handle {
    control: Sender<Control>,
    status: Arc<Mutex<MidiStatus>>,
    transfer: Transfer,
    thread: Option<JoinHandle<()>>,
    /// The output last passed to [`Handle::select`].
    chosen: Option<String>,
}

impl Handle {
    pub(crate) fn start(events: MidiEvents, hook: Option<SendHook>) -> Self {
        let (control, requests) = mpsc::channel();
        let (reply_sender, replies) = mpsc::channel();
        let status = Arc::new(Mutex::new(MidiStatus::Off));
        let transfer = Arc::new(Mutex::new((0, TransferStatus::Idle)));
        let worker = Worker {
            events,
            requests,
            status: Arc::clone(&status),
            transfer: Arc::clone(&transfer),
            hook,
            lister: None,
            input_lister: None,
            wanted: None,
            connection: None,
            is_virtual: false,
            input: None,
            heard: Arc::new(Heard::default()),
            replies,
            reply_sender,
            watch: Watch::default(),
            pending: Pending::default(),
            sounding: Sounding::default(),
            next_check: Instant::now() + PORT_CHECK,
            job: None,
            later: Vec::new(),
            cancelled: false,
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
            transfer,
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

    pub(crate) fn send_style(&mut self, target: KeyboardTarget, name: String, data: Vec<u8>) {
        let mut transfer = self.transfer.lock().unwrap_or_else(PoisonError::into_inner);
        transfer.0 += 1;
        transfer.1 = TransferStatus::Running(0.0);
        let job = Job {
            number: transfer.0,
            target,
            name,
            data,
        };
        if self.control.send(Control::SendStyle(job)).is_err() {
            transfer.1 = TransferStatus::Failed(TransferError::NotConnected);
        }
    }

    pub(crate) fn transfer(&self) -> TransferStatus {
        self.transfer
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .1
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

/// What the input's callback passes to the thread.
#[derive(Default)]
struct Heard {
    /// When the latest Active Sensing arrived, until the thread takes it.
    sensing: Mutex<Option<Instant>>,
    /// Whether SysEx messages go to the thread. Only during a transfer; otherwise they are
    /// dropped, so that nothing piles up.
    collecting: AtomicBool,
}

impl Heard {
    fn take_sensing(&self) -> Option<Instant> {
        self.sensing
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
    }
}

/// The thread's state. It owns the queue and the connections.
struct Worker {
    events: MidiEvents,
    requests: Receiver<Control>,
    status: Arc<Mutex<MidiStatus>>,
    transfer: Transfer,
    hook: Option<SendHook>,
    /// A client kept for the once-a-second check of the output list. Connecting consumes a
    /// `MidiOutput`, so connections use clients of their own.
    lister: Option<MidiOutput>,
    /// The same for the input list.
    input_lister: Option<MidiInput>,
    /// The output the interface chose.
    wanted: Option<String>,
    connection: Option<MidiOutputConnection>,
    /// Whether `connection` is the app's own virtual port, which is never in the port list.
    is_virtual: bool,
    /// The input with the same name as the chosen output, if there is one. It tells whether the
    /// keyboard is switched on, and carries its replies during a transfer.
    input: Option<MidiInputConnection<()>>,
    heard: Arc<Heard>,
    /// The SysEx messages from the input, while `heard.collecting` is set.
    replies: Receiver<Vec<u8>>,
    /// Cloned into each input's callback.
    reply_sender: Sender<Vec<u8>>,
    /// The Active Sensing seen from the chosen device.
    watch: Watch,
    pending: Pending,
    sounding: Sounding,
    /// When to next check the port list.
    next_check: Instant,
    /// A transfer asked for and not yet started.
    job: Option<Job>,
    /// Requests that arrived during a transfer and wait for its end.
    later: Vec<Control>,
    /// Whether the running transfer was called off.
    cancelled: bool,
}

impl Worker {
    fn run(mut self) {
        loop {
            loop {
                let control = match self.requests.try_recv() {
                    Ok(control) => control,
                    Err(TryRecvError::Disconnected) => Control::Stop,
                    Err(TryRecvError::Empty) => break,
                };
                if !self.obey(control) {
                    return;
                }
            }
            if let Some(job) = self.job.take() {
                self.run_transfer(job);
                for control in std::mem::take(&mut self.later) {
                    if !self.obey(control) {
                        return;
                    }
                }
                continue;
            }

            self.keep_watch();
            self.take_events();
            self.send_due();
            self.nap();
        }
    }

    /// Acts on a request from the interface. Returns false when the thread should end.
    fn obey(&mut self, control: Control) -> bool {
        match control {
            Control::Select(name) => self.select(name),
            Control::SendStyle(job) => self.job = Some(job),
            Control::Stop => {
                self.disconnect();
                return false;
            }
        }
        true
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
        self.watch = Watch::default();
        self.wanted = name;
        if self.wanted.is_some() {
            self.connect();
        } else {
            self.set_status(MidiStatus::Off);
        }
    }

    /// Tries to open the chosen output, and the input of the same name if it is not open yet,
    /// and sets the status to match.
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
            if self.input.is_none() {
                self.open_input(&name);
            }
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

    /// Sends the silencing messages and closes the connections, if they are open.
    fn disconnect(&mut self) {
        if let Some(mut connection) = self.connection.take() {
            self.silence(&mut connection);
            connection.close();
        }
        self.close_input();
        self.pending.clear();
        self.sounding = Sounding::default();
    }

    /// Sends a note-off for each sounding note and "All notes off", and forgets the notes.
    fn silence(&mut self, connection: &mut MidiOutputConnection) {
        for message in self.sounding.silence() {
            if let Err(e) = connection.send(&message) {
                log::warn!("MIDI out: could not silence the output: {e}");
                break;
            }
        }
    }

    /// Drops the output connection without silencing it, because the output has gone. The input
    /// stays open, to hear the keyboard come back.
    fn lose_connection(&mut self) {
        self.connection = None;
        self.pending.clear();
        self.sounding = Sounding::default();
        self.next_check = Instant::now() + PORT_CHECK;
        self.set_status(MidiStatus::Missing);
    }

    /// Opens the input with exactly this name, if there is one. Failing is fine: without an input
    /// the sender works as before, and a transfer reports [`TransferError::NoInput`].
    fn open_input(&mut self, name: &str) {
        self.close_input();
        let mut input = match MidiInput::new(CLIENT_NAME) {
            Ok(input) => input,
            Err(e) => {
                log::debug!("MIDI out: no input client: {e}");
                return;
            }
        };
        input.ignore(Ignore::None);
        let Some(port) = input
            .ports()
            .into_iter()
            .find(|port| input.port_name(port).is_ok_and(|n| n == name))
        else {
            log::debug!("MIDI out: no input named {name:?}");
            return;
        };
        let heard = Arc::clone(&self.heard);
        let replies = self.reply_sender.clone();
        let callback = move |_: u64, message: &[u8], _: &mut ()| match message.first() {
            Some(&ACTIVE_SENSING) => {
                *heard.sensing.lock().unwrap_or_else(PoisonError::into_inner) =
                    Some(Instant::now());
            }
            Some(0xF0) if heard.collecting.load(Ordering::Relaxed) => {
                let _ = replies.send(message.to_vec());
            }
            _ => {}
        };
        match input.connect(&port, CLIENT_NAME, callback, ()) {
            Ok(connection) => {
                log::debug!("MIDI out: listening to {name:?}");
                self.input = Some(connection);
            }
            Err(e) => log::debug!("MIDI out: cannot open the input {name:?}: {e}"),
        }
    }

    /// Closes the input, if one is open, and forgets what it heard.
    fn close_input(&mut self) {
        if let Some(input) = self.input.take() {
            input.close();
        }
        self.heard.take_sensing();
        self.watch.input_closed();
    }

    /// Follows the keyboard's Active Sensing at every turn, and once a second checks that the
    /// output and the input are still listed, or tries again to open the output.
    fn keep_watch(&mut self) {
        if let Some(at) = self.heard.take_sensing() {
            if self.watch.presence(at) == Presence::Unknown {
                log::info!("MIDI out: {:?} sends Active Sensing", self.wanted);
            }
            self.watch.heard(at);
        }
        let now = Instant::now();
        let check = now >= self.next_check;
        if check {
            self.next_check = now + PORT_CHECK;
        }
        let Some(name) = self.wanted.clone() else {
            return;
        };
        if check && self.input.is_some() && !self.input_listed(&name) {
            log::debug!("MIDI out: the input {name:?} is gone");
            self.close_input();
        }
        match self.watch.action(self.connection.is_some(), check, now) {
            Action::Keep => {
                if check && self.connection.is_some() && !self.is_virtual && !self.is_listed(&name)
                {
                    self.lose_connection();
                }
            }
            Action::Drop => {
                log::info!("MIDI out: no Active Sensing from {name:?}; it seems switched off");
                self.lose_connection();
            }
            Action::Open => self.connect(),
            // The old input may be dead for good, even once the keyboard is back, so listen
            // on a new one.
            Action::Listen => {
                self.open_input(&name);
                self.set_status(MidiStatus::Missing);
            }
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

    fn input_listed(&mut self, name: &str) -> bool {
        if self.input_lister.is_none() {
            self.input_lister = MidiInput::new(CLIENT_NAME).ok();
        }
        match &self.input_lister {
            Some(lister) => lister
                .ports()
                .iter()
                .any(|port| lister.port_name(port).is_ok_and(|n| n == name)),
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

    /// Runs a style transfer to its end and sets the transfer status, unless a newer request
    /// has replaced it.
    fn run_transfer(&mut self, job: Job) {
        let result = self.transfer_style(&job);
        // The keyboard may have paused Active Sensing; give it a second from now.
        self.watch.excuse(Instant::now());
        self.next_check = Instant::now() + PORT_CHECK;
        let status = match result {
            Ok(name) => {
                log::info!("style transfer: saved {name}");
                TransferStatus::Done(name)
            }
            Err(e) => {
                log::warn!("style transfer failed: {e:?}");
                TransferStatus::Failed(e)
            }
        };
        set_transfer(&self.transfer, job.number, status);
    }

    fn transfer_style(&mut self, job: &Job) -> Result<String, TransferError> {
        let Some(name) = self.wanted.clone() else {
            return Err(TransferError::NotConnected);
        };
        if self.connection.is_none() {
            return Err(TransferError::NotConnected);
        }
        if self.is_virtual {
            return Err(TransferError::NoInput);
        }
        if self.input.is_none() {
            self.open_input(&name);
        }
        if self.input.is_none() {
            return Err(TransferError::NoInput);
        }
        log::info!(
            "style transfer: {} ({} bytes) to {name:?}",
            job.name,
            job.data.len()
        );
        // Silence the output but keep it open; the engine's messages wait out the transfer.
        if let Some(mut connection) = self.connection.take() {
            self.silence(&mut connection);
            self.connection = Some(connection);
        }
        self.pending.clear();
        while self.replies.try_recv().is_ok() {}
        self.heard.collecting.store(true, Ordering::Relaxed);
        self.cancelled = false;

        let transfer = Arc::clone(&self.transfer);
        let number = job.number;
        let mut progress = |share: f32| {
            set_transfer(&transfer, number, TransferStatus::Running(share));
        };
        let result = transfer::send_style(self, job.target, &job.name, &job.data, &mut progress);

        self.heard.collecting.store(false, Ordering::Relaxed);
        while self.replies.try_recv().is_ok() {}
        result
    }

    /// During a transfer: discards the engine's messages and looks at the interface's requests.
    /// Returns true if the transfer must end now, because the app is closing or the output
    /// changed.
    fn poll_during_transfer(&mut self) -> bool {
        while self.events.pop().is_ok() {}
        let mut cancel = false;
        loop {
            match self.requests.try_recv() {
                Ok(Control::Select(name)) => {
                    if name != self.wanted {
                        cancel = true;
                    }
                    self.later.push(Control::Select(name));
                }
                Ok(Control::SendStyle(job)) => self.job = Some(job),
                Ok(Control::Stop) | Err(TryRecvError::Disconnected) => {
                    self.later.push(Control::Stop);
                    return true;
                }
                Err(TryRecvError::Empty) => return cancel,
            }
        }
    }
}

impl Link for Worker {
    fn request(&mut self, message: &[u8]) -> Result<Vec<u8>, Fault> {
        while self.replies.try_recv().is_ok() {}
        log::trace!("style transfer >> {}", show(message));
        let Some(connection) = self.connection.as_mut() else {
            return Err(Fault::SendFailed("the output is closed".to_owned()));
        };
        if let Err(e) = connection.send(message) {
            log::warn!("MIDI out: send failed: {e}");
            self.lose_connection();
            return Err(Fault::SendFailed(e.to_string()));
        }
        // Once called off, messages still go out (the session's end) but nobody waits.
        if self.cancelled {
            return Err(Fault::Cancelled);
        }
        let deadline = Instant::now() + REPLY_TIMEOUT;
        loop {
            let now = Instant::now();
            if now >= deadline {
                log::trace!("style transfer: no reply");
                return Err(Fault::NoReply);
            }
            match self
                .replies
                .recv_timeout((deadline - now).min(TRANSFER_POLL))
            {
                Ok(reply) => {
                    log::trace!("style transfer << {}", show(&reply));
                    return Ok(reply);
                }
                // The thread holds a sender, so the channel never disconnects.
                Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => {}
            }
            if self.poll_during_transfer() {
                self.cancelled = true;
                return Err(Fault::Cancelled);
            }
        }
    }
}

/// Sets the transfer status if request `number` is still the latest.
fn set_transfer(transfer: &Transfer, number: u64, status: TransferStatus) {
    let mut transfer = transfer.lock().unwrap_or_else(PoisonError::into_inner);
    if transfer.0 == number {
        transfer.1 = status;
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
