//! The sender thread and its connections to the output, to the input of the same name, and to
//! a second input for drum hits when one is chosen, through `midir`. Not built on Android.
//!
//! The input of the output's name serves the switch-off check, the replies during a style
//! transfer and, when the input choice points at it, drum hits. Windows lets only one connection
//! hold a port, so it is opened once for all three. An input chosen by another name gets a
//! connection of its own.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, TryRecvError};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use dm_audio::{LiveHits, MidiEvents};
use midir::{Ignore, MidiInput, MidiInputConnection, MidiOutput, MidiOutputConnection};

use crate::input::Notes;
use crate::liveness::{Action, Presence, Watch};
use crate::schedule::{Pending, Sounding, output_list, should_send};
use crate::transfer::{self, Fault, Link, REPLY_TIMEOUT, show};
use crate::{InputChoice, KeyboardTarget, MidiStatus, SendHook, TransferError, TransferStatus};

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
    SelectInput(InputChoice),
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
    input_status: Arc<Mutex<MidiStatus>>,
    tap: Arc<Tap>,
    /// The input last passed to [`Handle::select_input`].
    chosen_input: InputChoice,
}

impl Handle {
    pub(crate) fn start(events: MidiEvents, hook: Option<SendHook>) -> Self {
        let (control, requests) = mpsc::channel();
        let (reply_sender, replies) = mpsc::channel();
        let status = Arc::new(Mutex::new(MidiStatus::Off));
        let transfer = Arc::new(Mutex::new((0, TransferStatus::Idle)));
        let input_status = Arc::new(Mutex::new(MidiStatus::Off));
        let tap = Arc::new(Tap::default());
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
            input_id: 0,
            input_fault: None,
            heard: Arc::new(Heard::default()),
            tap: Arc::clone(&tap),
            input_status: Arc::clone(&input_status),
            input_choice: InputChoice::default(),
            note_input: None,
            note_fault: None,
            last_id: 0,
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
            input_status,
            tap,
            chosen_input: InputChoice::default(),
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

    pub(crate) fn listen(&mut self, hits: LiveHits) {
        *self.tap.hits.lock().unwrap_or_else(PoisonError::into_inner) = Some(hits);
    }

    pub(crate) fn set_listening(&mut self, on: bool) {
        self.tap.set_listening(on);
    }

    pub(crate) fn select_input(&mut self, choice: InputChoice) {
        if choice == self.chosen_input {
            return;
        }
        self.chosen_input = choice.clone();
        // As for the output: the old status must not stand for the new choice.
        *self
            .input_status
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = MidiStatus::Off;
        let _ = self.control.send(Control::SelectInput(choice));
    }

    pub(crate) fn input_status(&self) -> MidiStatus {
        self.input_status
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

/// The input names the system reports now, sorted, without the app's own virtual port.
pub(crate) fn inputs() -> Vec<String> {
    let mut names: Vec<String> = match MidiInput::new(CLIENT_NAME) {
        Ok(input) => input
            .ports()
            .iter()
            .filter_map(|port| input.port_name(port).ok())
            .filter(|name| VIRTUAL != Some(name.as_str()))
            .collect(),
        Err(e) => {
            log::warn!("MIDI in: cannot list the inputs: {e}");
            Vec::new()
        }
    };
    names.sort();
    names.dedup();
    names
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

/// What the inputs' callbacks share for drum hits.
#[derive(Default)]
struct Tap {
    /// Where hits go, once the interface has given a queue. Locked by a callback for each hit,
    /// and otherwise only by [`Handle::listen`]; neither holds it for longer than a push.
    hits: Mutex<Option<LiveHits>>,
    /// Which connection's notes count, by the number the thread gave it; 0 for none.
    source: AtomicU64,
    /// Whether the app should sound the hits: false when they come from the output's device.
    sound: AtomicBool,
    /// Moves on each change of source or choice. A callback that sees it move forgets its
    /// channel.
    epoch: AtomicU64,
    /// Whether hits go into the queue at all. Off by default. While off, a callback pushes
    /// nothing and takes no channel.
    listening: AtomicBool,
}

impl Tap {
    /// Lets hits through or stops them. Turning on takes a channel afresh.
    fn set_listening(&self, on: bool) {
        if self.listening.swap(on, Ordering::AcqRel) != on && on {
            self.epoch.fetch_add(1, Ordering::AcqRel);
        }
    }
}

/// The part of an input's callback that turns notes into hits.
struct Listener {
    /// The number of the connection this callback belongs to.
    id: u64,
    tap: Arc<Tap>,
    notes: Notes,
    /// The `tap.epoch` the channel was taken in.
    epoch: u64,
}

impl Listener {
    fn new(id: u64, tap: Arc<Tap>) -> Self {
        let epoch = tap.epoch.load(Ordering::Acquire);
        Listener {
            id,
            tap,
            notes: Notes::new(Instant::now()),
            epoch,
        }
    }

    /// Handles a message on midir's thread. Never waits for more than a push into the queue.
    fn hear(&mut self, timestamp: u64, message: &[u8]) {
        let now = Instant::now();
        if self.tap.source.load(Ordering::Acquire) != self.id
            || !self.tap.listening.load(Ordering::Acquire)
        {
            // Still learn the clock, so that it is ready if this input becomes the source or
            // listening turns on.
            self.notes.read(&[], timestamp, now, false);
            return;
        }
        let epoch = self.tap.epoch.load(Ordering::Acquire);
        if epoch != self.epoch {
            self.epoch = epoch;
            self.notes.release_channel();
        }
        let sound = self.tap.sound.load(Ordering::Acquire);
        let Some(hit) = self.notes.read(message, timestamp, now, sound) else {
            return;
        };
        let mut hits = self.tap.hits.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(hits) = hits.as_mut()
            && hits.push(hit).is_err()
        {
            log::debug!("MIDI in: the hit queue is full; dropped key {}", hit.key);
        }
    }
}

/// An input opened for drum hits alone, because it is not the output's device.
struct NoteInput {
    connection: MidiInputConnection<()>,
    name: String,
    id: u64,
}

/// Where the drum hits come from, by the choices made.
#[derive(Debug, PartialEq, Eq)]
enum Route {
    /// From nowhere.
    Off,
    /// From the input of the output's name, which the switch-off check also uses.
    Shared,
    /// From an input of their own, with this name.
    Own(String),
}

impl Route {
    /// Whether the app should sound the hits. Not when they come from the output's own device,
    /// which already plays them, so nothing sounds twice.
    fn sound(&self) -> bool {
        !matches!(self, Route::Shared)
    }
}

/// Where the drum hits come from, for the chosen output and input.
fn route(output: Option<&str>, choice: &InputChoice) -> Route {
    let output = output.filter(|&name| VIRTUAL != Some(name));
    match choice {
        InputChoice::None => Route::Off,
        InputChoice::SameAsOutput if output.is_some() => Route::Shared,
        InputChoice::SameAsOutput => Route::Off,
        InputChoice::Named(name) if output == Some(name.as_str()) => Route::Shared,
        InputChoice::Named(name) => Route::Own(name.clone()),
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
    /// The number of `input`'s connection, or 0.
    input_id: u64,
    /// Why `input` would not open, the last time it was tried.
    input_fault: Option<String>,
    heard: Arc<Heard>,
    tap: Arc<Tap>,
    input_status: Arc<Mutex<MidiStatus>>,
    /// The input the interface chose for drum hits.
    input_choice: InputChoice,
    /// The input for drum hits when it is not `input`.
    note_input: Option<NoteInput>,
    /// Why the input for drum hits would not open, the last time it was tried.
    note_fault: Option<String>,
    /// The number given to the latest input connection.
    last_id: u64,
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
            Control::SelectInput(choice) => self.select_input(choice),
            Control::SendStyle(job) => self.job = Some(job),
            Control::Stop => {
                self.disconnect();
                self.close_note_input();
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
        // Before the output's input opens: an input of its own for drum hits must close first if
        // it has the new output's name, since only one connection may hold a port.
        self.follow_notes(true);
        if self.wanted.is_some() {
            self.connect();
        } else {
            self.set_status(MidiStatus::Off);
        }
        self.follow_notes(false);
    }

    fn select_input(&mut self, choice: InputChoice) {
        if choice == self.input_choice {
            return;
        }
        self.input_choice = choice;
        // A new choice takes a channel afresh, even on the same connection.
        self.tap.epoch.fetch_add(1, Ordering::AcqRel);
        self.follow_notes(true);
    }

    /// Where the drum hits should come from now.
    fn route(&self) -> Route {
        route(self.wanted.as_deref(), &self.input_choice)
    }

    /// Opens and closes the inputs for drum hits to match the choices, points the callbacks at
    /// the right one, and sets the input status. Cheap unless `open` is set: only then may an
    /// input be opened, or its presence checked.
    fn follow_notes(&mut self, open: bool) {
        let route = self.route();
        if let Some(note_input) = &self.note_input {
            let wanted = matches!(&route, Route::Own(name) if *name == note_input.name);
            if !wanted {
                self.close_note_input();
            } else if open && !input_listed(&mut self.input_lister, &note_input.name) {
                log::info!("MIDI in: {:?} is gone", note_input.name);
                self.close_note_input();
            }
        }
        if open {
            match &route {
                Route::Own(name) if self.note_input.is_none() => {
                    let name = name.clone();
                    self.open_note_input(&name);
                }
                Route::Shared if self.input.is_none() => {
                    if let Some(name) = self.wanted.clone() {
                        self.open_input(&name);
                    }
                }
                _ => {}
            }
        }

        let (source, status) = match &route {
            Route::Off => (0, MidiStatus::Off),
            Route::Shared => (
                self.input_id,
                input_state(self.input.is_some(), &self.input_fault),
            ),
            Route::Own(_) => {
                let id = self.note_input.as_ref().map_or(0, |input| input.id);
                (id, input_state(self.note_input.is_some(), &self.note_fault))
            }
        };
        self.tap.sound.store(route.sound(), Ordering::Release);
        if self.tap.source.swap(source, Ordering::AcqRel) != source {
            self.tap.epoch.fetch_add(1, Ordering::AcqRel);
        }
        self.set_input_status(status, &route);
    }

    fn set_input_status(&self, status: MidiStatus, route: &Route) {
        let mut current = self
            .input_status
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if *current != status {
            let name = match route {
                Route::Own(name) => Some(name.as_str()),
                _ => self.wanted.as_deref(),
            };
            match &status {
                MidiStatus::Off => log::info!("MIDI in: off"),
                MidiStatus::Connected => log::info!("MIDI in: listening to {name:?}"),
                MidiStatus::Missing => log::info!("MIDI in: {name:?} is not there"),
                MidiStatus::Failed(reason) => {
                    log::warn!("MIDI in: cannot open {name:?}: {reason}")
                }
            }
            *current = status;
        }
    }

    /// Opens an input of its own for drum hits.
    fn open_note_input(&mut self, name: &str) {
        self.note_fault = None;
        let mut input = match MidiInput::new(CLIENT_NAME) {
            Ok(input) => input,
            Err(e) => {
                self.note_fault = Some(e.to_string());
                return;
            }
        };
        input.ignore(Ignore::SysexAndTime);
        let Some(port) = input
            .ports()
            .into_iter()
            .find(|port| input.port_name(port).is_ok_and(|n| n == name))
        else {
            return;
        };
        let id = self.next_id();
        let mut listener = Listener::new(id, Arc::clone(&self.tap));
        let callback = move |timestamp: u64, message: &[u8], _: &mut ()| {
            listener.hear(timestamp, message);
        };
        match input.connect(&port, CLIENT_NAME, callback, ()) {
            Ok(connection) => {
                self.note_input = Some(NoteInput {
                    connection,
                    name: name.to_owned(),
                    id,
                });
            }
            Err(e) => self.note_fault = Some(e.to_string()),
        }
    }

    fn close_note_input(&mut self) {
        if let Some(input) = self.note_input.take() {
            input.connection.close();
        }
    }

    /// A number for a new input connection, never 0.
    fn next_id(&mut self) -> u64 {
        self.last_id += 1;
        self.last_id
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
        self.input_fault = None;
        let mut input = match MidiInput::new(CLIENT_NAME) {
            Ok(input) => input,
            Err(e) => {
                log::debug!("MIDI out: no input client: {e}");
                self.input_fault = Some(e.to_string());
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
        let id = self.next_id();
        let mut listener = Listener::new(id, Arc::clone(&self.tap));
        let callback = move |timestamp: u64, message: &[u8], _: &mut ()| {
            match message.first() {
                Some(&ACTIVE_SENSING) => {
                    *heard.sensing.lock().unwrap_or_else(PoisonError::into_inner) =
                        Some(Instant::now());
                }
                Some(0xF0) if heard.collecting.load(Ordering::Relaxed) => {
                    let _ = replies.send(message.to_vec());
                }
                _ => {}
            }
            listener.hear(timestamp, message);
        };
        match input.connect(&port, CLIENT_NAME, callback, ()) {
            Ok(connection) => {
                log::debug!("MIDI out: listening to {name:?}");
                self.input = Some(connection);
                self.input_id = id;
            }
            Err(e) => {
                log::debug!("MIDI out: cannot open the input {name:?}: {e}");
                self.input_fault = Some(e.to_string());
            }
        }
    }

    /// Closes the input, if one is open, and forgets what it heard.
    fn close_input(&mut self) {
        if let Some(input) = self.input.take() {
            input.close();
        }
        self.input_id = 0;
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
        self.watch_output(check, now);
        self.follow_notes(check);
    }

    /// The part of [`Worker::keep_watch`] for the output and the input of the same name.
    fn watch_output(&mut self, check: bool, now: Instant) {
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
        input_listed(&mut self.input_lister, name)
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
                Ok(Control::SelectInput(choice)) => self.later.push(Control::SelectInput(choice)),
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

/// Whether an input with this name is listed, through a client kept for the purpose. True when
/// no client can be made, since then the list cannot be read.
fn input_listed(lister: &mut Option<MidiInput>, name: &str) -> bool {
    if lister.is_none() {
        *lister = MidiInput::new(CLIENT_NAME).ok();
    }
    match lister {
        Some(lister) => lister
            .ports()
            .iter()
            .any(|port| lister.port_name(port).is_ok_and(|n| n == name)),
        None => true,
    }
}

/// The status of an input for drum hits that should be open: whether it is, and if not, why.
fn input_state(open: bool, fault: &Option<String>) -> MidiStatus {
    match (open, fault) {
        (true, _) => MidiStatus::Connected,
        (false, Some(reason)) => MidiStatus::Failed(reason.clone()),
        (false, None) => MidiStatus::Missing,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hits_sound_unless_they_come_from_the_outputs_own_device() {
        let named = |name: &str| InputChoice::Named(name.to_owned());
        let keyboard = Some("Keyboard");

        // The output's own input: the keyboard plays its own sound.
        let shared = route(keyboard, &InputChoice::SameAsOutput);
        assert_eq!(shared, Route::Shared);
        assert!(!shared.sound());
        let same_name = route(keyboard, &named("Keyboard"));
        assert_eq!(same_name, Route::Shared);
        assert!(!same_name.sound());

        // Another input, such as drum pads: the app sounds the hits.
        let pads = route(keyboard, &named("Pads"));
        assert_eq!(pads, Route::Own("Pads".to_owned()));
        assert!(pads.sound());
        let pads_alone = route(None, &named("Pads"));
        assert_eq!(pads_alone, Route::Own("Pads".to_owned()));
        assert!(pads_alone.sound());

        // Nothing to listen to.
        assert_eq!(route(None, &InputChoice::SameAsOutput), Route::Off);
        assert_eq!(route(keyboard, &InputChoice::None), Route::Off);
    }

    /// A tap with a queue, and a listener that is its source.
    fn tapped() -> (Arc<Tap>, Listener, rtrb::Consumer<dm_audio::LiveHit>) {
        let (hits, heard) = rtrb::RingBuffer::new(16);
        let tap = Arc::new(Tap::default());
        *tap.hits.lock().unwrap() = Some(hits);
        tap.source.store(1, Ordering::Release);
        tap.sound.store(true, Ordering::Release);
        (Arc::clone(&tap), Listener::new(1, tap), heard)
    }

    #[test]
    fn hits_go_through_only_while_listening() {
        let (tap, mut listener, mut heard) = tapped();
        // Off by default: nothing is pushed.
        listener.hear(0, &[0x90, 36, 100]);
        assert!(heard.pop().is_err());

        tap.set_listening(true);
        listener.hear(1000, &[0x90, 38, 100]);
        assert_eq!(heard.pop().map(|hit| hit.key), Ok(38));

        tap.set_listening(false);
        listener.hear(2000, &[0x90, 42, 100]);
        assert!(heard.pop().is_err());
    }

    #[test]
    fn hits_while_off_take_no_channel() {
        let (tap, mut listener, mut heard) = tapped();
        // Played on channel 1 while off.
        listener.hear(0, &[0x90, 36, 100]);
        tap.set_listening(true);
        // Channel 10 still takes the channel once listening.
        listener.hear(1000, &[0x99, 38, 100]);
        assert_eq!(heard.pop().map(|hit| hit.key), Ok(38));
        listener.hear(2000, &[0x90, 36, 100]);
        assert!(heard.pop().is_err());

        // Turning off and on again takes a channel afresh.
        tap.set_listening(false);
        tap.set_listening(true);
        listener.hear(3000, &[0x90, 36, 100]);
        assert_eq!(heard.pop().map(|hit| hit.key), Ok(36));
    }

    #[test]
    fn a_listener_that_is_not_the_source_pushes_nothing() {
        let (tap, mut listener, mut heard) = tapped();
        tap.set_listening(true);
        tap.source.store(2, Ordering::Release);
        listener.hear(0, &[0x90, 36, 100]);
        assert!(heard.pop().is_err());
    }
}
